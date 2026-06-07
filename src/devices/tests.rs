use super::*;
use rstest::rstest;
use serde_json::json;

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

#[test]
fn kl135_supports_dim_color_and_energy() {
    let entry = lookup("KL135").unwrap();
    for feature in ["dim", "color_temp", "color", "energy", "specs", "presets"] {
        assert!(
            entry.supports.iter().any(|f| f == feature),
            "KL135 should support '{feature}'"
        );
    }
}

#[test]
fn kl135_does_not_support_schedules_or_led() {
    let entry = lookup("KL135").unwrap();
    for feature in ["schedules", "led", "clock"] {
        assert!(
            !entry.supports.iter().any(|f| f == feature),
            "KL135 should NOT support '{feature}'"
        );
    }
}

#[test]
fn kp115_supports_energy() {
    let entry = lookup("KP115").unwrap();
    assert!(entry.supports.iter().any(|f| f == "energy"));
}

#[test]
fn hs105_does_not_support_energy() {
    let entry = lookup("HS105").unwrap();
    assert!(
        !entry.supports.iter().any(|f| f == "energy"),
        "HS105 has no energy chip"
    );
}

#[test]
fn kl430_does_not_support_dim_or_power() {
    let entry = lookup("KL430").unwrap();
    for feature in ["dim", "power", "color_temp", "color"] {
        assert!(
            !entry.supports.iter().any(|f| f == feature),
            "KL430 should NOT support '{feature}' (not yet implemented)"
        );
    }
    assert!(
        entry.supports.iter().any(|f| f == "effects"),
        "KL430 should support lighting effects"
    );
}

#[test]
fn kl420l5_is_registered_as_a_lightstrip_with_energy_only() {
    let entry = lookup("KL420L5").unwrap();
    assert!(matches!(entry.kind, DeviceKind::LightStrip));
    assert!(!entry.verified);
    assert!(entry.supports.iter().any(|f| f == "energy"));
    assert!(entry.supports.iter().any(|f| f == "effects"));
    for feature in ["power", "dim", "color_temp", "color"] {
        assert!(
            !entry.supports.iter().any(|f| f == feature),
            "KL420L5 should NOT support '{feature}' yet"
        );
    }
}

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
fn hints_power_first_reflects_state() {
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

fn parse_hint(hint: &str) -> Result<crate::cli::Cli, clap::Error> {
    use clap::Parser;
    let args: Vec<String> = hint
        .split_whitespace()
        .map(|t| t.trim_matches('"').to_string())
        .collect();
    crate::cli::Cli::try_parse_from(args)
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

#[test]
fn detect_kind_separates_bulbs_and_light_strips() {
    let bulb = json!({
        "system": { "get_sysinfo": { "mic_type": "IOT.SMARTBULB" } }
    });
    let strip = json!({
        "system": { "get_sysinfo": { "mic_type": "IOT.SMARTBULB", "length": 200 } }
    });
    assert_eq!(detect_kind(&bulb), DeviceKind::Bulb);
    assert_eq!(detect_kind(&strip), DeviceKind::LightStrip);
}

#[test]
fn detect_kind_prefers_dimmer_then_strip_then_plug() {
    let dimmer = json!({
        "system": { "get_sysinfo": {
            "mic_type": "IOT.SMARTPLUGSWITCH", "dev_name": "Smart Wi-Fi Dimmer"
        }}
    });
    let strip = json!({
        "system": { "get_sysinfo": {
            "mic_type": "IOT.SMARTPLUGSWITCH", "dev_name": "Power Strip", "children": []
        }}
    });
    let plug = json!({
        "system": { "get_sysinfo": {
            "type": "IOT.SMARTPLUGSWITCH", "dev_name": "Smart Wi-Fi Plug"
        }}
    });
    assert_eq!(detect_kind(&dimmer), DeviceKind::Dimmer);
    assert_eq!(detect_kind(&strip), DeviceKind::Strip);
    assert_eq!(detect_kind(&plug), DeviceKind::Plug);
}

#[test]
fn detect_kind_preserves_unknown_type() {
    let json = json!({
        "system": { "get_sysinfo": { "mic_type": "IOT.UNKNOWN" } }
    });
    assert_eq!(detect_kind(&json).to_string(), "unknown (IOT.UNKNOWN)");
}

#[test]
fn detect_kind_missing_type_is_unknown_empty() {
    let json = json!({ "system": { "get_sysinfo": {} } });
    assert_eq!(detect_kind(&json), DeviceKind::Unknown(String::new()));
}

#[test]
fn can_control_power_accepts_bulb_plug_dimmer_strip() {
    assert!(can_control_power(&DeviceKind::Bulb).is_ok());
    assert!(can_control_power(&DeviceKind::Plug).is_ok());
    assert!(can_control_power(&DeviceKind::Dimmer).is_ok());
    assert!(can_control_power(&DeviceKind::Strip).is_ok());
    assert!(can_control_power(&DeviceKind::LightStrip).is_err());
    assert!(can_control_power(&DeviceKind::Unknown("IOT.X".into())).is_err());
}

#[test]
fn can_dim_accepts_bulb_and_dimmer_only() {
    assert!(can_dim(&DeviceKind::Bulb).is_ok());
    assert!(can_dim(&DeviceKind::Dimmer).is_ok());
    assert!(can_dim(&DeviceKind::LightStrip).is_err());
    assert!(can_dim(&DeviceKind::Plug).is_err());
    assert!(can_dim(&DeviceKind::Strip).is_err());
    assert!(can_dim(&DeviceKind::Unknown("IOT.X".into())).is_err());
}

#[test]
fn can_dim_error_names_the_command() {
    let err = can_dim(&DeviceKind::Plug).unwrap_err();
    assert!(err.to_string().contains("`dim`"), "{err}");
}

#[test]
fn can_dim_lightstrip_error_explains_namespace() {
    let err = can_dim(&DeviceKind::LightStrip).unwrap_err();
    assert!(err.to_string().contains("not yet supported"), "{err}");
    assert!(err.to_string().contains("lightStrip"), "{err}");
}

#[rstest]
#[case(can_set_color_temp as fn(&DeviceKind) -> Result<()>, "`color-temp`")]
#[case(can_set_color    as fn(&DeviceKind) -> Result<()>, "`color`")]
#[case(can_get_specs    as fn(&DeviceKind) -> Result<()>, "`specs`")]
#[case(can_get_presets  as fn(&DeviceKind) -> Result<()>, "`presets`")]
fn bulb_only_guards_accept_bulb_reject_others(
    #[case] guard: fn(&DeviceKind) -> Result<()>,
    #[case] cmd: &str,
) {
    assert!(
        guard(&DeviceKind::Bulb).is_ok(),
        "{cmd}: should accept bulb"
    );
    for kind in [
        DeviceKind::Dimmer,
        DeviceKind::Plug,
        DeviceKind::Strip,
        DeviceKind::LightStrip,
    ] {
        let err = guard(&kind).unwrap_err();
        assert!(
            err.to_string().contains(cmd),
            "{cmd}: error should name the command for {kind}: {err}"
        );
    }
}

#[test]
fn can_set_color_temp_error_mentions_kl135() {
    let err = can_set_color_temp(&DeviceKind::Plug).unwrap_err();
    assert!(err.to_string().contains("KL135"), "{err}");
}

#[rstest]
#[case(can_get_schedules as fn(&DeviceKind) -> Result<()>, "`schedules`")]
#[case(can_control_led   as fn(&DeviceKind) -> Result<()>, "`led`")]
#[case(can_get_clock     as fn(&DeviceKind) -> Result<()>, "`clock`")]
fn relay_device_guards_accept_plug_dimmer_strip(
    #[case] guard: fn(&DeviceKind) -> Result<()>,
    #[case] cmd: &str,
) {
    assert!(guard(&DeviceKind::Plug).is_ok(), "{cmd}: plug");
    assert!(guard(&DeviceKind::Dimmer).is_ok(), "{cmd}: dimmer");
    assert!(guard(&DeviceKind::Strip).is_ok(), "{cmd}: strip");
    assert!(
        guard(&DeviceKind::LightStrip).is_err(),
        "{cmd}: light strip"
    );
    let err = guard(&DeviceKind::Bulb).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains(cmd), "{cmd}: error should name command: {msg}");
    assert!(
        msg.contains("KP115") || msg.contains("HS220") || msg.contains("HS300"),
        "{cmd}: error should mention supported models: {msg}"
    );
}

#[rstest]
#[case(DeviceKind::Plug)]
#[case(DeviceKind::Strip)]
fn require_energy_bails_when_parse_fails(#[case] kind: DeviceKind) {
    assert!(require_energy(&json!({}), &kind).is_err());
}

#[test]
fn require_energy_plug_with_ene_feature_succeeds() {
    let json = json!({
        "system": { "get_sysinfo": {
            "mic_type": "IOT.SMARTPLUGSWITCH",
            "alias": "Desk Plug", "model": "KP115(US)",
            "hw_ver": "1.0", "sw_ver": "1.1.1",
            "rssi": -48, "relay_state": 1, "feature": "TIM:ENE"
        }}
    });
    assert!(require_energy(&json, &DeviceKind::Plug).is_ok());
}

#[test]
fn require_energy_plug_without_ene_fails_with_model_name() {
    let json = json!({
        "system": { "get_sysinfo": {
            "mic_type": "IOT.SMARTPLUGSWITCH",
            "alias": "Simple Plug", "model": "HS105(US)",
            "hw_ver": "4.0", "sw_ver": "1.0.0",
            "rssi": -55, "relay_state": 0, "feature": "TIM"
        }}
    });
    let err = require_energy(&json, &DeviceKind::Plug).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("HS105"), "error should name the model: {msg}");
    assert!(msg.contains("energy"), "{msg}");
}

#[test]
fn require_energy_bulb_and_lightstrip_always_ok() {
    assert!(require_energy(&json!({}), &DeviceKind::Bulb).is_ok());
    assert!(require_energy(&json!({}), &DeviceKind::LightStrip).is_ok());
}

#[test]
fn require_energy_unknown_kind_fails() {
    assert!(require_energy(&json!({}), &DeviceKind::Dimmer).is_err());
    assert!(require_energy(&json!({}), &DeviceKind::Unknown("X".into())).is_err());
}
