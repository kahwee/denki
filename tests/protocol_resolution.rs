//! Offline CLI checks: use capability rejection to prove routing without device IO.
#![cfg(target_os = "linux")]
use std::process::Command;

#[test]
fn direct_ips_use_saved_protocol_and_scan_rejects_conflicts_before_network() {
    let config = tempfile::tempdir().unwrap();
    let dir = config.path().join("denki");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("hosts.json");
    let run = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_denki"))
            .env("XDG_CONFIG_HOME", config.path())
            .args(args)
            .output()
            .unwrap();
        assert!(!output.status.success());
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()
    };
    for protocol in ["klap", "tapo"] {
        let contents =
            serde_json::json!({"plug": {"ip":"192.0.2.1", "protocol":protocol}}).to_string();
        std::fs::write(&path, &contents).unwrap();
        let by_alias = run(&["--json", "energy-daily", "plug"]);
        let by_ip = run(&["--json", "energy-daily", "192.0.2.1"]);
        assert_eq!(by_alias["error"]["code"], "unsupported_operation");
        assert_eq!(by_ip["error"]["code"], "unsupported_operation");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), contents);
    }
    let contents = r#"{"one":{"ip":"192.0.2.1","protocol":"klap"},"two":{"ip":"192.0.2.1","protocol":"tapo"}}"#;
    std::fs::write(&path, contents).unwrap();
    for args in [
        vec!["--json", "info", "192.0.2.1"],
        vec!["--json", "scan", "--timeout", "0"],
    ] {
        assert_eq!(run(&args)["error"]["code"], "ambiguous_protocol");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), contents);
    }
    std::fs::write(&path, "{broken").unwrap();
    let failure = run(&["--json", "info", "192.0.2.1"]);
    assert!(
        failure["error"]["message"]
            .as_str()
            .unwrap()
            .contains("malformed")
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{broken");
}
