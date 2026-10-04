//! A corrupt registry must stop scan before discovery or persistence.
#[cfg(target_os = "linux")]
#[test]
fn scan_preserves_corrupt_registry_and_reports_error() {
    let config = tempfile::tempdir().unwrap();
    let dir = config.path().join("denki");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("hosts.json");
    let original = "{broken registry";
    std::fs::write(&path, original).unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_denki"))
        .env("XDG_CONFIG_HOME", config.path())
        .args(["scan", "--timeout", "0"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("malformed"), "{stderr}");
    assert_eq!(std::fs::read_to_string(path).unwrap(), original);
}
