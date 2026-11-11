//! Bounded subprocess capture and process-group cleanup, including inherited pipes.
use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use rustix::process::{Pid, Signal, kill_process_group};
use std::fs::File;
use std::io::Read;
use std::os::fd::OwnedFd;
use std::os::unix::process::CommandExt as _;
use std::process::{Child, ChildStdin, Command, ExitStatus, Output, Stdio};
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

struct Capture {
    pipe: File,
    bytes: Vec<u8>,
    eof: bool,
}

impl Capture {
    fn new(pipe: impl Into<OwnedFd>) -> Self {
        let pipe = File::from(pipe.into());
        let flags = fcntl_getfl(&pipe).unwrap();
        fcntl_setfl(&pipe, flags | OFlags::NONBLOCK).unwrap();
        Self {
            pipe,
            bytes: Vec::new(),
            eof: false,
        }
    }

    fn drain(&mut self) {
        // A continuously writing child must not prevent deadline checks.
        let mut buffer = [0; 8192];
        for _ in 0..16 {
            match self.pipe.read(&mut buffer) {
                Ok(0) => {
                    self.eof = true;
                    break;
                }
                Ok(n) => self.bytes.extend_from_slice(&buffer[..n]),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => panic!("cannot capture child output: {error}"),
            }
        }
    }
}

pub(super) struct Process {
    child: Child,
    group: Option<Pid>,
    stdout: Capture,
    stderr: Capture,
}

impl Process {
    pub(super) fn spawn(command: &mut Command) -> Self {
        let mut child = command
            .process_group(0)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let group = Pid::from_raw(child.id() as i32).unwrap();
        Self {
            group: Some(group),
            stdout: Capture::new(child.stdout.take().unwrap()),
            stderr: Capture::new(child.stderr.take().unwrap()),
            child,
        }
    }

    pub(super) fn take_stdin(&mut self) -> ChildStdin {
        self.child
            .stdin
            .take()
            .expect("configure piped stdin before spawning")
    }

    pub(super) fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>> {
        self.drain();
        self.child.try_wait()
    }

    pub(super) fn kill(&mut self) -> std::io::Result<()> {
        if let Some(group) = self.group {
            kill_process_group(group, Signal::KILL)?;
        }
        Ok(())
    }

    pub(super) fn id(&self) -> u32 {
        self.child.id()
    }

    fn drain(&mut self) {
        self.stdout.drain();
        self.stderr.drain();
    }

    pub(super) fn first_line(&mut self, timeout: Duration) -> Vec<u8> {
        let deadline = Instant::now() + timeout;
        loop {
            self.drain();
            if let Some(end) = self.stdout.bytes.iter().position(|byte| *byte == b'\n') {
                return self.stdout.bytes[..=end].to_vec();
            }
            if self.stdout.eof && !self.stdout.bytes.is_empty() {
                return self.stdout.bytes.clone();
            }
            assert!(!self.stdout.eof, "CLI exited without a sample");
            assert!(
                Instant::now() < deadline,
                "CLI did not emit its first sample before the deadline"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    pub(super) fn finish(mut self, timeout: Duration) -> Output {
        let deadline = Instant::now() + timeout;
        let status = loop {
            self.drain();
            if let Some(status) = self.child.try_wait().unwrap()
                && self.stdout.eof
                && self.stderr.eof
            {
                break status;
            }
            if Instant::now() >= deadline {
                self.cleanup();
                self.drain();
                panic!(
                    "CLI or inherited output pipes timed out after {timeout:?}\nstdout: {}\nstderr: {}",
                    String::from_utf8_lossy(&self.stdout.bytes),
                    String::from_utf8_lossy(&self.stderr.bytes)
                );
            }
            std::thread::sleep(Duration::from_millis(2));
        };
        Output {
            status,
            stdout: std::mem::take(&mut self.stdout.bytes),
            stderr: std::mem::take(&mut self.stderr.bytes),
        }
    }

    fn cleanup(&mut self) {
        if let Some(group) = self.group.take() {
            let _ = kill_process_group(group, Signal::KILL);
            let _ = self.child.kill();
            // Never block indefinitely in Drop, even on an uninterruptible child.
            let deadline = Instant::now() + Duration::from_millis(250);
            while matches!(self.child.try_wait(), Ok(None)) && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(2));
            }
        }
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        self.cleanup();
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
        let mut process = Process::spawn(Command::new("sleep").arg("60"));
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

#[test]
fn inherited_pipes_and_panics_do_not_leave_running_descendants() {
    for mode in ["parent-exit", "timeout", "panic"] {
        let script = if mode == "parent-exit" {
            "sleep 5 & echo $!; echo stderr-sentinel >&2; exit 0"
        } else {
            "sleep 5 & echo $!; echo stderr-sentinel >&2; wait"
        };
        let mut process = Process::spawn(Command::new("sh").args(["-c", script]));
        let leader = process.id();
        let descendant = String::from_utf8(process.first_line(Duration::from_secs(2)))
            .unwrap()
            .trim()
            .parse::<u32>()
            .unwrap();
        let started = Instant::now();
        let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if mode == "panic" {
                panic!("assertion after receiving first sample");
            }
            process.finish(Duration::from_millis(40));
        }))
        .expect_err("inherited pipes must be covered by the deadline");
        assert!(started.elapsed() < Duration::from_secs(1), "{mode}");
        if mode != "panic" {
            let message = failure.downcast_ref::<String>().unwrap();
            assert!(message.contains("stderr-sentinel"), "{message}");
            assert!(message.contains(&descendant.to_string()), "{message}");
        }
        assert!(!std::path::Path::new(&format!("/proc/{leader}")).exists());
        let deadline = Instant::now() + Duration::from_millis(500);
        loop {
            // An orphan may briefly remain a zombie until PID 1 reaps it.
            let running = std::fs::read_to_string(format!("/proc/{descendant}/stat"))
                .is_ok_and(|stat| !stat.split_once(") ").unwrap().1.starts_with('Z'));
            if !running {
                break;
            }
            assert!(Instant::now() < deadline, "descendant survived {mode}");
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}
