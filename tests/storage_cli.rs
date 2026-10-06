//! Cross-process locking, crash release, and secure credential replacement.
#![cfg(target_os = "linux")]
use serde_json::{Value, json};
use std::fs::{File, OpenOptions};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
fn command(config: &std::path::Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_denki"));
    command
        .env("XDG_CONFIG_HOME", config)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}
#[test]
fn concurrent_alias_commands_preserve_every_edit_and_never_publish_partial_json() {
    let config = tempfile::tempdir().unwrap();
    let dir = config.path().join("denki");
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("hosts.json");
    std::fs::write(&path, b"{}").unwrap();
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(dir.join("hosts.json.lock"))
        .unwrap();
    lock.lock().unwrap();
    let mut children = Vec::new();
    for i in 0..16 {
        children.push(
            command(
                config.path(),
                &[
                    "alias",
                    &format!("device {i}"),
                    &format!("192.0.2.{}", i + 1),
                    "--json",
                ],
            )
            .spawn()
            .unwrap(),
        );
    }
    std::thread::sleep(Duration::from_millis(150));
    assert!(
        children.iter_mut().all(|c| c.try_wait().unwrap().is_none()),
        "writers bypassed the held lock"
    );
    lock.unlock().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let value: Value = serde_json::from_slice(&std::fs::read(&path).unwrap())
            .expect("a reader observed partial JSON");
        assert!(value.is_object());
        if children.iter_mut().all(|c| c.try_wait().unwrap().is_some()) {
            break;
        }
        if Instant::now() > deadline {
            for child in &mut children {
                let _ = child.kill();
            }
            panic!("writers did not finish");
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    for child in children {
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let saved: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved.as_object().unwrap().len(), 16);
    let mut children = Vec::new();
    for i in 0..8 {
        children.push(
            command(
                config.path(),
                &["unalias", &format!("device {i}"), "--json"],
            )
            .spawn()
            .unwrap(),
        );
    }
    for i in 16..24 {
        children.push(
            command(
                config.path(),
                &[
                    "alias",
                    &format!("device {i}"),
                    &format!("192.0.2.{}", i + 1),
                    "--json",
                ],
            )
            .spawn()
            .unwrap(),
        );
    }
    for child in children {
        assert!(child.wait_with_output().unwrap().status.success());
    }
    let saved: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved.as_object().unwrap().len(), 16);
    for i in 0..8 {
        assert!(saved.get(format!("device {i}")).is_none());
    }
    for i in 8..24 {
        assert_eq!(
            saved[format!("device {i}")]["ip"],
            format!("192.0.2.{}", i + 1)
        );
    }
}
#[test]
fn credential_creation_is_private_under_permissive_umask_and_atomic_on_replacement() {
    use std::os::unix::fs::PermissionsExt;
    let config = tempfile::tempdir().unwrap();
    let path = config.path().join("denki/credentials.json");
    for pass in ["synthetic-password-one", "synthetic-password-two"] {
        if path.exists() {
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o666)).unwrap();
        }
        let output = Command::new("sh")
            .args([
                "-c",
                "umask 000; exec \"$@\"",
                "denki-test",
                env!("CARGO_BIN_EXE_denki"),
                "login",
                "test@example.invalid",
                pass,
                "--json",
            ])
            .env("XDG_CONFIG_HOME", config.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!String::from_utf8_lossy(&output.stdout).contains(pass));
        assert!(!String::from_utf8_lossy(&output.stderr).contains(pass));
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let saved: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            saved,
            json!({"tapo_user":"test@example.invalid","tapo_pass":pass})
        );
    }
}
// A helper process holds the same OS lock and gets killed by the parent test.
#[test]
fn lock_holder_helper() {
    let Some(path) = std::env::var_os("DENKI_TEST_LOCK") else {
        return;
    };
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    file.lock().unwrap();
    std::fs::write(std::env::var_os("DENKI_TEST_READY").unwrap(), b"ready").unwrap();
    std::thread::park();
}
#[test]
fn killed_writer_releases_lock_without_deleting_sidecar() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("hosts.json.lock");
    File::create(&path).unwrap();
    let ready = dir.path().join("ready");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "lock_holder_helper"])
        .env("DENKI_TEST_LOCK", &path)
        .env("DENKI_TEST_READY", &ready)
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !ready.exists() {
        if Instant::now() > deadline {
            child.kill().unwrap();
            panic!("helper failed to lock");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    assert!(file.try_lock().is_err());
    child.kill().unwrap();
    child.wait().unwrap();
    file.try_lock()
        .expect("OS must release a killed process's lock");
    assert!(path.exists());
}
