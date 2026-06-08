use super::support::parse_hint;
use crate::devices::{hint_for, hints, lookup};

#[test]
fn verified_devices_are_marked() {
    for model in ["KL135", "KP115", "HS105", "HS110", "P125"] {
        let entry = lookup(model).unwrap();
        assert!(entry.verified, "{model} should be marked verified = true");
    }
}

#[test]
fn unverified_devices_are_marked() {
    for model in ["HS220", "KL430"] {
        let entry = lookup(model).unwrap();
        assert!(!entry.verified, "{model} should be marked verified = false");
    }
}

#[test]
fn p125_uses_klap_protocol() {
    let entry = lookup("P125").unwrap();
    assert_eq!(entry.protocol.as_deref(), Some("klap"));
}

#[test]
fn kasa_devices_have_no_protocol_field() {
    for model in ["KL135", "KP115", "HS105"] {
        let entry = lookup(model).unwrap();
        assert!(
            entry.protocol.is_none(),
            "{model}: Kasa devices should not set protocol"
        );
    }
}

#[test]
fn hints_power_first_reflect_state() {
    let entry = lookup("KP115").unwrap();
    assert_eq!(hints(entry, "plug", true)[0], "denki off \"plug\"");
    assert_eq!(hints(entry, "plug", false)[0], "denki on \"plug\"");
}

#[test]
fn hints_kp115_includes_energy_but_not_outlets() {
    let entry = lookup("KP115").unwrap();
    let h = hints(entry, "plug", false);
    assert!(h.iter().any(|s| s.contains("energy")));
    assert!(!h.iter().any(|s| s.contains("outlet")));
}

#[test]
fn hints_hs105_includes_schedules_but_not_energy() {
    let entry = lookup("HS105").unwrap();
    let h = hints(entry, "plug", false);
    assert!(h.iter().any(|s| s.contains("schedules")));
    assert!(!h.iter().any(|s| s.contains("energy")));
}

#[test]
fn hints_hs300_includes_outlets() {
    let entry = lookup("HS300").unwrap();
    let h = hints(entry, "strip", false);
    assert!(h.iter().any(|s| s.contains("outlets")));
}

#[test]
fn hints_kl135_includes_color_and_dim() {
    let entry = lookup("KL135").unwrap();
    let h = hints(entry, "bulb", false);
    assert!(h.iter().any(|s| s.contains("color-temp")));
    assert!(h.iter().any(|s| s.contains("dim")));
    assert!(h.iter().any(|s| s.contains("color")));
}

#[test]
fn hint_dim_parses() {
    let hint = hint_for("dim", "bulb").unwrap();
    assert!(parse_hint(&hint).is_ok(), "hint failed to parse: {hint}");
}

#[test]
fn hint_color_temp_parses() {
    let hint = hint_for("color_temp", "bulb").unwrap();
    assert!(parse_hint(&hint).is_ok(), "hint failed to parse: {hint}");
}

#[test]
fn hint_color_parses() {
    let hint = hint_for("color", "bulb").unwrap();
    assert!(parse_hint(&hint).is_ok(), "hint failed to parse: {hint}");
}

#[test]
fn hint_energy_parses() {
    let hint = hint_for("energy", "plug").unwrap();
    assert!(parse_hint(&hint).is_ok(), "hint failed to parse: {hint}");
}

#[test]
fn hint_schedules_parses() {
    let hint = hint_for("schedules", "plug").unwrap();
    assert!(parse_hint(&hint).is_ok(), "hint failed to parse: {hint}");
}

#[test]
fn hint_led_parses() {
    let hint = hint_for("led", "plug").unwrap();
    assert!(parse_hint(&hint).is_ok(), "hint failed to parse: {hint}");
}

#[test]
fn hint_clock_parses() {
    let hint = hint_for("clock", "plug").unwrap();
    assert!(parse_hint(&hint).is_ok(), "hint failed to parse: {hint}");
}

#[test]
fn hint_outlets_parses() {
    let hint = hint_for("outlets", "strip").unwrap();
    assert!(parse_hint(&hint).is_ok(), "hint failed to parse: {hint}");
}

#[test]
fn hint_specs_parses() {
    let hint = hint_for("specs", "bulb").unwrap();
    assert!(parse_hint(&hint).is_ok(), "hint failed to parse: {hint}");
}

#[test]
fn hint_presets_parses() {
    let hint = hint_for("presets", "bulb").unwrap();
    assert!(parse_hint(&hint).is_ok(), "hint failed to parse: {hint}");
}

#[test]
fn hint_outlet_rename_parses() {
    use clap::Parser;
    let args = ["denki", "outlet-rename", "strip", "2", "NAS"];
    assert!(crate::cli::Cli::try_parse_from(args).is_ok());
}

#[test]
fn hint_for_unknown_feature_returns_none() {
    assert!(hint_for("nonexistent", "x").is_none());
    assert!(hint_for("power", "x").is_none());
    assert!(hint_for("", "x").is_none());
}
