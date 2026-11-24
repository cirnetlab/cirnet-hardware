use std::io::{self, Read, Write};

fn main() -> io::Result<()> {
    let (mut stdin, mut stdout) = (io::stdin().lock(), io::stdout().lock());
    let mut prefix = [0; 4];
    stdin.read_exact(&mut prefix)?;
    let mut request = vec![0; u32::from_le_bytes(prefix) as usize];
    stdin.read_exact(&mut request)?;
    let generation = u64::from_le_bytes(request[12..20].try_into().unwrap());
    // Acknowledge only the Windows startup preview's empty CNP1 request.
    let readiness = cfg!(windows)
        && request.len() == 52
        && request.starts_with(b"CNP1")
        && request[20..28] == [0; 8]
        && request[28..32] == 10_000_u32.to_le_bytes()
        && request[32..] == [0; 20];
    let reply_generation = if readiness {
        generation
    } else {
        generation.wrapping_sub(1)
    };
    let mut reply = request[..12].to_vec();
    reply.extend_from_slice(&reply_generation.to_le_bytes());
    reply.extend_from_slice(&[0; 5]);
    stdout.write_all(&(reply.len() as u32).to_le_bytes())?;
    stdout.write_all(&reply)?;
    stdout.flush()?;
    // Wait for the supervised parent to close or reap this child.
    let _ = stdin.read(&mut prefix);
    Ok(())
}
