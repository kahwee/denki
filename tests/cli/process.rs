//! Bounded subprocess capture. Readers drain both pipes while the child runs.
use std::io::{BufRead, BufReader, Read};
use std::process::{Child, ChildStdin, Command, ExitStatus, Output, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub(super) const CLI_TIMEOUT: Duration = Duration::from_secs(10);

pub(super) trait CommandExt {
    fn bounded_output(&mut self) -> Output;
    fn spawn_bounded(&mut self) -> Process;
}

impl CommandExt for Command {
    fn bounded_output(&mut self) -> Output {
        Process::spawn(self.stdin(Stdio::null())).finish(CLI_TIMEOUT)
    }

    fn spawn_bounded(&mut self) -> Process {
        Process::spawn(self)
    }
}

pub(super) struct Process {
    child: Child,
    stdout: Option<JoinHandle<Vec<u8>>>,
    stderr: Option<JoinHandle<Vec<u8>>>,
    first: Receiver<Vec<u8>>,
}

impl Process {
    pub(super) fn spawn(command: &mut Command) -> Self {
        let mut child = command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let (send, first) = mpsc::channel();
        Self {
            child,
            stdout: Some(std::thread::spawn(move || {
                let mut reader = BufReader::new(stdout);
                let mut bytes = Vec::new();
                reader.read_until(b'\n', &mut bytes).unwrap();
                let _ = send.send(bytes.clone());
                reader.read_to_end(&mut bytes).unwrap();
                bytes
            })),
            stderr: Some(std::thread::spawn(move || {
                let mut reader = stderr;
                let mut bytes = Vec::new();
                reader.read_to_end(&mut bytes).unwrap();
                bytes
            })),
            first,
        }
    }

    pub(super) fn take_stdin(&mut self) -> ChildStdin {
        self.child
            .stdin
            .take()
            .expect("configure piped stdin before spawning")
    }

    pub(super) fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }

    pub(super) fn kill(&mut self) -> std::io::Result<()> {
        self.child.kill()
    }

    pub(super) fn id(&self) -> u32 {
        self.child.id()
    }

    pub(super) fn first_line(&self, timeout: Duration) -> Vec<u8> {
        let line = self
            .first
            .recv_timeout(timeout)
            .expect("CLI did not emit its first sample before the deadline");
        assert!(!line.is_empty(), "CLI exited without a sample");
        line
    }

    pub(super) fn finish(mut self, timeout: Duration) -> Output {
        let started = Instant::now();
        let mut timed_out = false;
        let status = loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                break status;
            }
            if started.elapsed() >= timeout {
                timed_out = true;
                let _ = self.child.kill();
                break self.child.wait().unwrap();
            }
            std::thread::sleep(Duration::from_millis(2));
        };
        let stdout = self.stdout.take().unwrap().join().unwrap();
        let stderr = self.stderr.take().unwrap().join().unwrap();
        assert!(
            !timed_out,
            "CLI timed out after {timeout:?}\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&stdout),
            String::from_utf8_lossy(&stderr)
        );
        Output {
            status,
            stdout,
            stderr,
        }
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        // Also runs if a sample assertion or signal delivery panics.
        let _ = self.child.kill();
        let _ = self.child.wait();
        for reader in [&mut self.stdout, &mut self.stderr] {
            if let Some(reader) = reader.take() {
                let _ = reader.join();
            }
        }
    }
}

#[test]
fn drains_both_pipes_and_preserves_failure_output() {
    let output = Process::spawn(Command::new("sh").args([
        "-c",
        "head -c 131072 /dev/zero; head -c 131072 /dev/zero >&2; exit 7",
    ]))
    .finish(Duration::from_secs(3));
    assert_eq!(output.status.code(), Some(7));
    assert_eq!(output.stdout, vec![0; 131072]);
    assert_eq!(output.stderr, vec![0; 131072]);
}

#[test]
fn deadlines_and_unwinding_reap_children() {
    for first_sample in [false, true] {
        let process = Process::spawn(Command::new("sleep").arg("60"));
        let pid = process.id();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if first_sample {
                process.first_line(Duration::from_millis(30));
            } else {
                process.finish(Duration::from_millis(30));
            }
        }));
        assert!(result.is_err());
        assert!(
            !std::path::Path::new(&format!("/proc/{pid}")).exists(),
            "child survived timeout"
        );
    }
}
