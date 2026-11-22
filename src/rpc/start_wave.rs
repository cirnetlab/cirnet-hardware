use super::qpc_ns;
use crate::pb;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;
use tokio::sync::watch;
use tokio::time::{Instant, timeout_at};
use tonic::metadata::{Ascii, MetadataValue};
use tonic::{Code, Request, Status};

pub(super) const HEADER: &str = "x-cirnet-start-wave";
const MEMBER_COUNT: usize = 4;
const ALL_MEMBERS: u8 = (1 << MEMBER_COUNT) - 1;
const ARRIVAL_LIMIT: Duration = Duration::from_secs(2);
const MAX_WAVES: usize = 1024;

#[derive(Default)]
pub(super) struct StartWaves(Mutex<HashMap<String, Arc<Wave>>>);

struct Wave {
    members: [String; MEMBER_COUNT],
    deadline: Instant,
    state: Mutex<WaveState>,
    changed: watch::Sender<()>,
}

#[derive(Default)]
struct WaveState {
    arrived: u8,
    ready: u8,
    outcome: Option<Result<u64, Code>>,
}

struct Pending(Arc<Wave>);

impl Drop for Pending {
    fn drop(&mut self) {
        let _ = self.0.finish(Err(Code::Cancelled));
    }
}

fn is_id(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

impl StartWaves {
    pub(super) async fn enter(
        &self,
        request: &Request<pb::Target>,
    ) -> Result<Option<MetadataValue<Ascii>>, Status> {
        let entered = qpc_ns();
        let values = request.metadata().get_all(HEADER);
        let mut values = values.iter();
        let Some(value) = values.next() else {
            return Ok(None);
        };
        let entered = entered.ok_or_else(|| wave_status(Code::Unavailable))?;
        let invalid = || wave_status(Code::InvalidArgument);
        if values.next().is_some() || request.get_ref().station_id != "virtual-station-01" {
            return Err(invalid());
        }
        let metadata = value.to_str().map_err(|_| invalid())?;
        let mut fields = metadata.split(':');
        if fields.next() != Some("v1") {
            return Err(invalid());
        }
        let wave_id = fields.next().filter(|id| is_id(id)).ok_or_else(invalid)?;
        let mut members = [""; MEMBER_COUNT];
        for member in &mut members {
            *member = fields.next().filter(|id| is_id(id)).ok_or_else(invalid)?;
        }
        if fields.next().is_some() || !members.windows(2).all(|pair| pair[0] < pair[1]) {
            return Err(invalid());
        }
        let experiment = &request.get_ref().experiment_id;
        let wave = self.get(wave_id, members).map_err(wave_status)?;
        let Ok(index) = members.binary_search(&experiment.as_str()) else {
            let _ = wave.finish(Err(Code::FailedPrecondition));
            return Err(wave_status(Code::FailedPrecondition));
        };
        let mut changed = wave.changed.subscribe();
        let member = 1 << index;
        wave.arrive(members, member).map_err(wave_status)?;
        // Dropping a waiting RPC future aborts this wave before it can release.
        let _pending = Pending(wave.clone());
        let released = match timeout_at(wave.deadline, async {
            loop {
                if let Some(outcome) = wave.ready(member) {
                    break outcome;
                }
                changed.changed().await.map_err(|_| Code::Unavailable)?;
            }
        })
        .await
        {
            Ok(outcome) => outcome,
            Err(_) => wave.finish(Err(Code::DeadlineExceeded)),
        }
        .map_err(wave_status)?;
        format!("qpc-ns-v1:{wave_id}:{experiment}:{entered}:{released}")
            .parse()
            .map(Some)
            .map_err(|_| wave_status(Code::Internal))
    }

    fn get(&self, id: &str, members: [&str; MEMBER_COUNT]) -> Result<Arc<Wave>, Code> {
        let now = Instant::now();
        let mut waves = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(wave) = waves.get(id) {
            return Ok(wave.clone());
        }
        // A dedicated diagnostic lane admits at most 403 waves plus warmups.
        // Keep all closed IDs for its lifetime so a late peer cannot reopen one.
        if waves.len() >= MAX_WAVES {
            return Err(Code::ResourceExhausted);
        }
        let wave = Arc::new(Wave {
            members: members.map(str::to_owned),
            deadline: now + ARRIVAL_LIMIT,
            state: Mutex::default(),
            changed: watch::channel(()).0,
        });
        waves.insert(id.to_owned(), wave.clone());
        Ok(wave)
    }
}

impl Wave {
    fn arrive(&self, members: [&str; MEMBER_COUNT], member: u8) -> Result<(), Code> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(outcome) = state.outcome {
            return Err(outcome.err().unwrap_or(Code::FailedPrecondition));
        }
        if Instant::now() >= self.deadline {
            return self
                .latch(&mut state, Err(Code::DeadlineExceeded))
                .map(|_| ());
        }
        if self.members.iter().map(String::as_str).ne(members) || state.arrived & member != 0 {
            return self
                .latch(&mut state, Err(Code::FailedPrecondition))
                .map(|_| ());
        }
        state.arrived |= member;
        if state.arrived == ALL_MEMBERS {
            self.changed.send_replace(());
        }
        Ok(())
    }

    fn ready(&self, member: u8) -> Option<Result<u64, Code>> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(outcome) = state.outcome {
            return Some(outcome);
        }
        if Instant::now() >= self.deadline {
            return Some(self.latch(&mut state, Err(Code::DeadlineExceeded)));
        }
        if state.arrived != ALL_MEMBERS {
            return None;
        }
        // All live handler futures must observe the full roster before release.
        state.ready |= member;
        if state.ready == ALL_MEMBERS {
            let released = qpc_ns().ok_or(Code::Unavailable);
            let outcome = if Instant::now() >= self.deadline {
                Err(Code::DeadlineExceeded)
            } else {
                released
            };
            return Some(self.latch(&mut state, outcome));
        }
        None
    }

    fn finish(&self, outcome: Result<u64, Code>) -> Result<u64, Code> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        self.latch(&mut state, outcome)
    }

    fn latch(&self, state: &mut WaveState, outcome: Result<u64, Code>) -> Result<u64, Code> {
        if let Some(existing) = state.outcome {
            return existing;
        }
        state.outcome = Some(outcome);
        self.changed.send_replace(());
        outcome
    }
}

fn wave_status(code: Code) -> Status {
    Status::new(code, "Hardware Start rendezvous did not release")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_after_full_arrival_prevents_release() {
        let waves = StartWaves::default();
        let members = ["a", "b", "c", "d"];
        let wave = waves.get("wave", members).unwrap();
        let guard = Pending(wave.clone());
        for member in [1, 2, 4, 8] {
            assert_eq!(wave.arrive(members, member), Ok(()));
        }
        for member in [1, 2, 4] {
            assert_eq!(wave.ready(member), None);
        }
        drop(guard);
        assert_eq!(wave.ready(8), Some(Err(Code::Cancelled)));
        assert_eq!(wave.ready(1), Some(Err(Code::Cancelled)));
        assert_eq!(wave.arrive(members, 8), Err(Code::Cancelled));
    }
}
