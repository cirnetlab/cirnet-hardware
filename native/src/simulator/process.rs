use super::SimError;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Mutex, TryLockError,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, Sender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

// Cancellation can cross both the reply wait and the bounded process reap.
const POLL: Duration = Duration::from_millis(2);
const REAP_TIMEOUT: Duration = Duration::from_secs(1);
const MAX_REPLY: usize = 25 + 100_000 * 13 + 8 + 131_072 * 10;

#[derive(Clone)]
pub(super) struct Worker {
    executable: PathBuf,
    sha256: String,
}

impl Worker {
    pub(super) fn new(executable: PathBuf, sha256: &str) -> Result<Self, SimError> {
        if !executable.is_absolute()
            || sha256.len() != 64
            || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(SimError::BackendUnavailable);
        }
        let executable = executable
            .canonicalize()
            .map_err(|_| SimError::BackendUnavailable)?;
        let worker = Self {
            executable,
            sha256: sha256.to_ascii_lowercase(),
        };
        worker.verify()?;
        Ok(worker)
    }

    fn verify(&self) -> Result<(), SimError> {
        let mut binary = BufReader::new(open_file(&self.executable)?);
        let mut hasher = Sha256::new();
        let mut bytes = [0_u8; 16 * 1024];
        loop {
            let length = binary
                .read(&mut bytes)
                .map_err(|_| SimError::BackendUnavailable)?;
            if length == 0 {
                break;
            }
            hasher.update(&bytes[..length]);
        }
        if format!("{:x}", hasher.finalize()) != self.sha256 {
            return Err(SimError::BackendUnavailable);
        }
        Ok(())
    }
}

fn open_file(path: &Path) -> Result<File, SimError> {
    let file = File::open(path).map_err(|_| SimError::BackendUnavailable)?;
    if !file
        .metadata()
        .map_err(|_| SimError::BackendUnavailable)?
        .is_file()
    {
        return Err(SimError::BackendUnavailable);
    }
    Ok(file)
}

struct Request {
    frame: Vec<u8>,
    response: Sender<Result<Vec<u8>, SimError>>,
}

struct Connection {
    child: Child,
    requests: Option<Sender<Request>>,
    io: Option<JoinHandle<()>>,
}

impl Connection {
    fn new(worker: &Worker) -> Result<Self, SimError> {
        worker.verify()?;
        let mut command = Command::new(&worker.executable);
        command
            .arg("--serve")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = command.spawn().map_err(|_| SimError::BackendUnavailable)?;
        let mut input = child.stdin.take().expect("configured child stdin pipe");
        let mut output = BufReader::new(child.stdout.take().expect("configured child stdout pipe"));
        let (sender, receiver) = mpsc::channel::<Request>();
        let io = thread::Builder::new()
            .name("cirnet-cnp-io".into())
            .spawn(move || {
                while let Ok(Request { frame, response }) = receiver.recv() {
                    let result = input
                        .write_all(&frame)
                        .and_then(|_| input.flush())
                        .map_err(|_| SimError::DriverError)
                        .and_then(|_| read_frame(&mut output));
                    let failed = result.is_err();
                    if response.send(result).is_err() || failed {
                        break;
                    }
                }
            });
        let mut connection = Self {
            child,
            requests: Some(sender),
            io: io.ok(),
        };
        if connection.io.is_none() {
            connection.close()?;
            return Err(SimError::DriverError);
        }
        Ok(connection)
    }

    fn exchange(
        &self,
        frame: Vec<u8>,
        deadline: Instant,
        cancel: &AtomicBool,
        closed: &AtomicBool,
    ) -> Result<Vec<u8>, SimError> {
        check_live(deadline, cancel, closed)?;
        let (response, received) = mpsc::channel();
        self.requests
            .as_ref()
            .ok_or(SimError::DriverError)?
            .send(Request { frame, response })
            .map_err(|_| SimError::DriverError)?;
        receive_reply(received, deadline, cancel, closed)
    }

    fn close(&mut self) -> Result<(), SimError> {
        self.requests.take();
        let deadline = Instant::now() + REAP_TIMEOUT;
        if self
            .child
            .try_wait()
            .map_err(|_| SimError::SafeStateFailed)?
            .is_none()
        {
            if self.child.kill().is_err()
                && self
                    .child
                    .try_wait()
                    .map_err(|_| SimError::SafeStateFailed)?
                    .is_none()
            {
                return Err(SimError::SafeStateFailed);
            }
            while self
                .child
                .try_wait()
                .map_err(|_| SimError::SafeStateFailed)?
                .is_none()
            {
                if Instant::now() >= deadline {
                    return Err(SimError::SafeStateFailed);
                }
                thread::sleep(POLL);
            }
        }
        if let Some(io) = self.io.as_ref() {
            while !io.is_finished() {
                if Instant::now() >= deadline {
                    return Err(SimError::SafeStateFailed);
                }
                thread::sleep(POLL);
            }
        }
        if let Some(io) = self.io.take() {
            io.join().map_err(|_| SimError::SafeStateFailed)?;
        }
        Ok(())
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

fn read_frame(pipe: &mut impl Read) -> Result<Vec<u8>, SimError> {
    let mut prefix = [0; 4];
    pipe.read_exact(&mut prefix[..1])
        .map_err(|_| SimError::DriverError)?;
    pipe.read_exact(&mut prefix[1..])
        .map_err(|_| SimError::ProtocolMismatch)?;
    let size = u32::from_le_bytes(prefix) as usize;
    if !(25..=MAX_REPLY).contains(&size) {
        return Err(SimError::ProtocolMismatch);
    }
    let mut frame = vec![0; size + prefix.len()];
    frame[..prefix.len()].copy_from_slice(&prefix);
    pipe.read_exact(&mut frame[prefix.len()..])
        .map_err(|_| SimError::ProtocolMismatch)?;
    Ok(frame)
}

fn check_live(deadline: Instant, cancel: &AtomicBool, closed: &AtomicBool) -> Result<(), SimError> {
    if cancel.load(Ordering::Acquire) {
        return Err(SimError::Cancelled);
    }
    if closed.load(Ordering::Acquire) {
        return Err(SimError::BackendUnavailable);
    }
    if Instant::now() >= deadline {
        return Err(SimError::Timeout);
    }
    Ok(())
}

fn receive_reply(
    received: Receiver<Result<Vec<u8>, SimError>>,
    deadline: Instant,
    cancel: &AtomicBool,
    closed: &AtomicBool,
) -> Result<Vec<u8>, SimError> {
    loop {
        check_live(deadline, cancel, closed)?;
        match received.recv_timeout(deadline.saturating_duration_since(Instant::now()).min(POLL)) {
            Ok(result) => {
                check_live(deadline, cancel, closed)?;
                return result;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Err(SimError::DriverError),
        }
    }
}

#[derive(Default)]
pub(super) struct Lane {
    connection: Option<Connection>,
    fenced: bool,
}

impl Lane {
    pub(super) fn exchange(
        &mut self,
        worker: &Worker,
        frame: Vec<u8>,
        timeout: Duration,
        cancel: &AtomicBool,
        closed: &AtomicBool,
    ) -> Result<Vec<u8>, SimError> {
        let deadline = Instant::now() + timeout;
        check_live(deadline, cancel, closed)?;
        if let Some(connection) = self.connection.as_mut() {
            let dead = match connection.child.try_wait() {
                Ok(state) => state.is_some(),
                Err(_) => {
                    self.discard()?;
                    return Err(SimError::DriverError);
                }
            };
            if self.fenced || dead {
                self.discard()?;
            }
        }
        if self.connection.is_none() {
            self.connection = Some(Connection::new(worker)?);
        }
        let result = self
            .connection
            .as_ref()
            .expect("active lane")
            .exchange(frame, deadline, cancel, closed);
        if result.is_err() {
            self.discard()?;
        }
        result
    }

    pub(super) fn discard(&mut self) -> Result<(), SimError> {
        self.fenced = true;
        if let Some(connection) = &mut self.connection {
            connection.close()?;
            self.connection = None;
        }
        self.fenced = false;
        Ok(())
    }
}

pub(super) struct Pool(Vec<Mutex<Lane>>);

impl Pool {
    pub(super) fn new(slots: usize) -> Self {
        Self((0..slots).map(|_| Mutex::new(Lane::default())).collect())
    }

    pub(super) fn with_lane<T>(
        &self,
        call: impl FnOnce(&mut Lane) -> Result<T, SimError>,
    ) -> Result<T, SimError> {
        for lane in &self.0 {
            match lane.try_lock() {
                Ok(mut lane) => return call(&mut lane),
                Err(TryLockError::WouldBlock) => {}
                Err(TryLockError::Poisoned(_)) => return Err(SimError::SafeStateFailed),
            }
        }
        Err(SimError::ResourceConflict)
    }

    pub(super) fn close(&self) -> Result<(), SimError> {
        let mut result = Ok(());
        for lane in &self.0 {
            match lane.lock() {
                Ok(mut lane) => {
                    if let Err(error) = lane.discard() {
                        result = Err(error);
                    }
                }
                Err(poisoned) => {
                    let _ = poisoned.into_inner().discard();
                    result = Err(SimError::SafeStateFailed);
                }
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framing_rejects_oversize_truncated_and_closed_pipes() {
        assert_eq!(read_frame(&mut (&[][..])), Err(SimError::DriverError));
        assert_eq!(
            read_frame(&mut (&[25, 0][..])),
            Err(SimError::ProtocolMismatch)
        );
        assert_eq!(
            read_frame(&mut (&[255, 255, 255, 255][..])),
            Err(SimError::ProtocolMismatch)
        );
        assert_eq!(
            read_frame(&mut (&[25, 0, 0, 0, 1][..])),
            Err(SimError::ProtocolMismatch)
        );
    }
}
