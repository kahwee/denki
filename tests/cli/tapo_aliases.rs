//! Offline CLI coverage: protocol changes preserve the alias registry.
#![cfg(target_os = "linux")]

use std::process::Command;

#[test]
fn tapo_alias_migration_and_group_preview_are_offline() {
    let config = tempfile::tempdir().unwrap();
    let dir = config.path().join("denki");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("hosts.json");
    std::fs::write(
        &path,
        r#"{
        "Test Plug": {"ip":"192.0.2.1","protocol":"klap","device_id":"fixture-identity"},
        "Other Plug": {"ip":"192.0.2.2","protocol":"kasa"}
    }"#,
    )
    .unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_denki"))
            .env("XDG_CONFIG_HOME", config.path())
            .args(args)
            .output()
            .unwrap()
    };
    assert!(
        run(&["alias", "Test Plug", "192.0.2.1", "--tpap"])
            .status
            .success()
    );
    let saved = std::fs::read(&path).unwrap();
    let map: serde_json::Value = serde_json::from_slice(&saved).unwrap();
    assert_eq!(map.as_object().unwrap().len(), 2);
    assert_eq!(map["Test Plug"]["protocol"], "tapo");
    assert_eq!(map["Test Plug"]["device_id"], "fixture-identity");
    assert_eq!(map["Other Plug"]["protocol"], "kasa");
    assert_eq!(map["Other Plug"]["ip"], "192.0.2.2");

    let preview = run(&["group", "on", "Test Plug", "--dry-run"]);
    assert!(preview.status.success());
    assert!(String::from_utf8_lossy(&preview.stdout).contains("tapo"));
    assert!(
        !run(&["alias", "Test Plug", "192.0.2.1", "--tapo", "--klap"])
            .status
            .success()
    );
    assert_eq!(std::fs::read(&path).unwrap(), saved);
    for mode in ["--klap", "--tapo"] {
        assert!(
            run(&["alias", "Test Plug", "192.0.2.1", mode])
                .status
                .success()
        );
        let map: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(map["Test Plug"]["device_id"], "fixture-identity");
    }
    assert!(
        run(&["alias", "Test Plug", "192.0.2.3", "--tapo"])
            .status
            .success()
    );
    let map: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert!(map["Test Plug"]["device_id"].is_null());
    assert_eq!(map["Other Plug"]["ip"], "192.0.2.2");
}
