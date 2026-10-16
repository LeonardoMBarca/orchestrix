//! Private inherited startup stdout, using the existing user-restricted pipe
//! and overlapped cancellation primitives. No I/O thread or application endpoint.
use crate::{ipc, named_job};
use std::{process::Stdio, time::Duration};
pub const MAX_BYTES: usize = 2048;
pub enum PipeRead {
    Pending,
    Closed,
    Data(Vec<u8>),
}
pub struct StartupReader {
    stream: ipc::Stream,
    received: usize,
}
pub fn channel() -> ipc::Result<(StartupReader, Stdio)> {
    let scope = named_job::nonce().map_err(|_| ipc::Error::Identity)?;
    let mut listener = ipc::Listener::bind(&scope)?;
    let client = ipc::Stream::connect(&scope, Duration::from_secs(1))?;
    let server = listener
        .accept(Duration::from_secs(1))?
        .ok_or(ipc::Error::Deadline)?;
    let writer = client.startup_writer()?;
    Ok((
        StartupReader {
            stream: server,
            received: 0,
        },
        writer,
    ))
}
impl StartupReader {
    pub fn poll(&mut self, timeout: Duration) -> ipc::Result<PipeRead> {
        if self.received >= MAX_BYTES {
            return Err(ipc::Error::Budget);
        }
        match self
            .stream
            .startup_read_chunk(MAX_BYTES - self.received, timeout)
        {
            Ok(bytes) => {
                self.received += bytes.len();
                Ok(PipeRead::Data(bytes))
            }
            Err(ipc::Error::Deadline) => Ok(PipeRead::Pending),
            Err(ipc::Error::Closed) => Ok(PipeRead::Closed),
            Err(error) => Err(error),
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::{io::Write, process::Command, time::Instant};
    #[test]
    fn private_stdout_is_incremental_bounded_and_does_not_need_child_exit() {
        const FLAG: &str = "ORCHESTRIX_TEST_STARTUP_PIPE_CHILD";
        if std::env::var_os(FLAG).is_some() {
            std::io::stdout().write_all(b"first\n").unwrap();
            std::io::stdout().flush().unwrap();
            std::thread::sleep(Duration::from_millis(150));
            std::io::stdout().write_all(b"second\n").unwrap();
            std::io::stdout().flush().unwrap();
            return;
        }
        let (mut reader, writer) = channel().unwrap();
        assert!(matches!(
            reader.poll(Duration::from_millis(2)).unwrap(),
            PipeRead::Pending
        ));
        // --quiet still writes libtest framing. This tests raw bytes/ownership,
        // not the launch report's separately validated JSON schema.
        let mut child=Command::new(std::env::current_exe().unwrap()).args(["--exact","startup_pipe::tests::private_stdout_is_incremental_bounded_and_does_not_need_child_exit","--nocapture","--quiet"]).env(FLAG,"1").stdout(writer).stderr(Stdio::null()).spawn().unwrap();
        let start = Instant::now();
        let mut bytes = vec![];
        let mut observed_alive = false;
        loop {
            match reader.poll(Duration::from_millis(5)).unwrap() {
                PipeRead::Data(data) => {
                    bytes.extend(data);
                    if bytes.windows(6).any(|v| v == b"first\n")
                        && child.try_wait().unwrap().is_none()
                    {
                        observed_alive = true;
                    }
                }
                PipeRead::Closed => break,
                PipeRead::Pending => {}
            }
            assert!(start.elapsed() < Duration::from_secs(5));
        }
        assert!(child.wait().unwrap().success());
        assert!(observed_alive);
        assert!(bytes.windows(7).any(|v| v == b"second\n"));
        assert!(bytes.len() < MAX_BYTES);
    }
    #[test]
    fn dropping_an_unused_writer_closes_reader_and_poll_has_a_small_deadline() {
        let (mut reader, writer) = channel().unwrap();
        let start = Instant::now();
        assert!(matches!(
            reader.poll(Duration::from_millis(5)).unwrap(),
            PipeRead::Pending
        ));
        assert!(start.elapsed() < Duration::from_secs(1));
        drop(writer);
        assert!(matches!(
            reader.poll(Duration::from_millis(5)).unwrap(),
            PipeRead::Closed
        ));
    }
    #[test]
    fn excess_private_stdout_stops_at_the_total_budget() {
        const FLAG: &str = "ORCHESTRIX_TEST_STARTUP_PIPE_BUDGET_CHILD";
        if std::env::var_os(FLAG).is_some() {
            std::io::stdout().write_all(&[b'x'; MAX_BYTES * 2]).unwrap();
            std::io::stdout().flush().unwrap();
            return;
        }
        let (mut reader, writer) = channel().unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "startup_pipe::tests::excess_private_stdout_stops_at_the_total_budget",
                "--nocapture",
                "--quiet",
            ])
            .env(FLAG, "1")
            .stdout(writer)
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let started = Instant::now();
        let mut bytes = 0;
        loop {
            match reader.poll(Duration::from_millis(5)) {
                Ok(PipeRead::Data(data)) => bytes += data.len(),
                Ok(PipeRead::Pending) => {}
                Err(ipc::Error::Budget) => break,
                _ => panic!("budget must be reached before pipe closure"),
            }
            assert!(started.elapsed() < Duration::from_secs(5));
        }
        assert_eq!(bytes, MAX_BYTES);
        // Keep the reader alive while this bounded child finishes its framing.
        assert!(child.wait().unwrap().success());
    }
}
