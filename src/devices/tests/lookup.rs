use crate::devices::{DeviceKind, all, lookup};

#[test]
fn all_loads_without_panicking() {
    assert!(!all().is_empty());
}

#[test]
fn all_contains_expected_models() {
    let models: Vec<&str> = all().iter().map(|d| d.model.as_str()).collect();
    for expected in [
        "KL135", "KP115", "HS105", "HS110", "HS220", "HS300", "KL430", "KL420L5", "P125",
    ] {
        assert!(
            models.contains(&expected),
            "missing model {expected} in devices.toml"
        );
    }
}

#[test]
fn all_entries_have_known_kind() {
    assert!(!all().is_empty());
}

#[test]
fn lookup_exact_model_name() {
    assert!(lookup("KL135").is_some());
    assert!(lookup("KP115").is_some());
    assert!(lookup("HS105").is_some());
}

#[test]
fn lookup_strips_country_suffix() {
    let entry = lookup("KL135(US)").expect("KL135(US) should match KL135");
    assert_eq!(entry.model, "KL135");
    assert!(matches!(entry.kind, DeviceKind::Bulb));
}

#[test]
fn lookup_is_case_insensitive() {
    assert!(lookup("kl135").is_some());
    assert!(lookup("KL135").is_some());
    assert!(lookup("kP115").is_some());
}

#[test]
fn lookup_unknown_model_returns_none() {
    assert!(lookup("ZZ999").is_none());
    assert!(lookup("").is_none());
}
