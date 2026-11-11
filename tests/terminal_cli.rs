//! Offline terminal-boundary regressions; no device sockets are opened.
#![cfg(target_os = "linux")]
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

const HOSTILE: &str = "lamp\x1b]52;c;YQ==\x07\x1b[2J\r\n\u{9b}2J\u{202e}";

fn run(config: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_denki"))
        .env("XDG_CONFIG_HOME", config)
        .env("NO_COLOR", "1")
        .env_remove("CLICOLOR_FORCE")
        .args(args)
        .output()
        .unwrap()
}

fn safe(bytes: &[u8]) -> String {
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    for c in text.chars().filter(|c| *c != '\n') {
        assert!(
            denki::output::is_terminal_safe(&c.to_string()),
            "unsafe output: {text:?}"
        );
    }
    text
}

#[test]
fn stored_aliases_errors_and_dry_runs_escape_terminal_controls_but_keep_json_raw() {
    let config = tempfile::tempdir().unwrap();
    let add = run(config.path(), &["alias", HOSTILE, "192.0.2.1", "--json"]);
    assert!(add.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&add.stdout).unwrap()["data"]["alias"],
        HOSTILE
    );
    assert!(safe(&add.stderr).contains(&denki::output::terminal_text(HOSTILE)));
    let list = run(config.path(), &["aliases"]);
    assert!(list.status.success());
    assert!(safe(&list.stdout).contains(&denki::output::terminal_text(HOSTILE)));
    safe(&list.stderr);
    let group = run(config.path(), &["group", "off", HOSTILE, "--dry-run"]);
    assert!(group.status.success());
    assert!(safe(&group.stdout).contains(&denki::output::terminal_text(HOSTILE)));
    safe(&group.stderr);
    let remove = run(config.path(), &["unalias", HOSTILE]);
    assert!(remove.status.success());
    assert!(safe(&remove.stdout).contains(&denki::output::terminal_text(HOSTILE)));
    for command in ["info", "unalias"] {
        let result = run(config.path(), &[command, HOSTILE]);
        assert!(!result.status.success());
        safe(&result.stdout);
        assert!(safe(&result.stderr).contains(&denki::output::terminal_text(HOSTILE)));
    }
    let watch = run(config.path(), &["energy", "watch", HOSTILE, "--count", "1"]);
    assert!(!watch.status.success());
    assert!(safe(&watch.stdout).contains(&denki::output::terminal_text(HOSTILE)));
    safe(&watch.stderr);
}

#[test]
fn parse_errors_escape_hostile_values_without_changing_exit_status_or_json() {
    let config = tempfile::tempdir().unwrap();
    let result = run(config.path(), &["scan", "--timeout", HOSTILE]);
    assert_eq!(result.status.code(), Some(2));
    safe(&result.stdout);
    safe(&result.stderr);
    let result = run(config.path(), &["scan", "--timeout", HOSTILE, "--json"]);
    assert!(!result.status.success());
    let envelope: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(envelope["error"]["code"], "invalid_arguments");
    safe(&result.stderr);
}

#[test]
fn login_rejects_password_arguments_without_echoing_and_accepts_redirected_input() {
    use std::io::Write;
    use std::process::Stdio;
    let config = tempfile::tempdir().unwrap();
    for extra in [vec![], vec!["--json"]] {
        let mut args = vec![
            "login",
            "synthetic@example.invalid",
            "private-password-sentinel",
        ];
        args.extend(extra);
        let result = run(config.path(), &args);
        assert!(!result.status.success());
        assert!(!String::from_utf8_lossy(&result.stdout).contains("private-password-sentinel"));
        assert!(!String::from_utf8_lossy(&result.stderr).contains("private-password-sentinel"));
        assert!(!config.path().join("denki/credentials.json").exists());
    }
    let mut child = Command::new(env!("CARGO_BIN_EXE_denki"))
        .env("XDG_CONFIG_HOME", config.path())
        .args([
            "login",
            "synthetic@example.invalid",
            "--password-stdin",
            "--json",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"private-password-sentinel\n")
        .unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(result.status.success());
    assert!(!String::from_utf8_lossy(&result.stdout).contains("private-password-sentinel"));
    assert!(!String::from_utf8_lossy(&result.stderr).contains("private-password-sentinel"));
    let saved: Value = serde_json::from_slice(
        &std::fs::read(config.path().join("denki/credentials.json")).unwrap(),
    )
    .unwrap();
    assert!(saved.to_string().contains("private-password-sentinel"));
}

#[test]
fn legacy_group_targets_need_explicit_reenrollment_without_network_access() {
    let config = tempfile::tempdir().unwrap();
    let dir = config.path().join("denki");
    std::fs::create_dir(&dir).unwrap();
    for registry in [
        r#"{"office":"192.0.2.1"}"#,
        r#"{"office":{"ip":"192.0.2.1","protocol":"kasa"}}"#,
    ] {
        std::fs::write(dir.join("hosts.json"), registry).unwrap();
        let result = run(config.path(), &["group", "off", "office", "--json"]);
        assert!(!result.status.success());
        let value: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(value["error"]["code"], "alias_approval_required");
        assert_eq!(
            std::fs::read_to_string(dir.join("hosts.json")).unwrap(),
            registry
        );
        assert!(
            run(config.path(), &["group", "off", "office", "--dry-run"])
                .status
                .success()
        );
        assert!(
            run(config.path(), &["alias", "office", "192.0.2.1"])
                .status
                .success()
        );
        let saved: Value =
            serde_json::from_slice(&std::fs::read(dir.join("hosts.json")).unwrap()).unwrap();
        assert_eq!(saved["office"]["group_approved"], true);
    }
}

#[test]
fn tpap_policy_blocks_explicit_klap_before_any_connection() {
    let config = tempfile::tempdir().unwrap();
    assert!(
        run(config.path(), &["alias", "fixture", "192.0.2.1", "--klap"])
            .status
            .success()
    );
    let result = Command::new(env!("CARGO_BIN_EXE_denki"))
        .env("XDG_CONFIG_HOME", config.path())
        .env("DENKI_REQUIRE_TPAP", "1")
        .env("TAPO_USER", "synthetic@example.invalid")
        .env("TAPO_PASS", "synthetic-password")
        .args(["info", "fixture", "--json"])
        .output()
        .unwrap();
    assert!(!result.status.success());
    let value: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(value["error"]["code"], "authentication_policy");
    assert!(!String::from_utf8_lossy(&result.stdout).contains("synthetic-password"));
}
