use crate::devices::{
    DeviceKind, can_control_led, can_control_power, can_dim, can_get_clock, can_get_schedules,
    can_set_color, can_set_color_temp, require_energy,
};
use rstest::rstest;
use serde_json::json;

#[test]
fn can_control_power_accepts_supported_device_kinds() {
    assert!(can_control_power(&DeviceKind::Bulb).is_ok());
    assert!(can_control_power(&DeviceKind::Plug).is_ok());
    assert!(can_control_power(&DeviceKind::Dimmer).is_ok());
    assert!(can_control_power(&DeviceKind::Strip).is_ok());
    assert!(can_control_power(&DeviceKind::LightStrip).is_ok());
    assert!(can_control_power(&DeviceKind::Unknown("IOT.X".into())).is_err());
}

#[test]
fn can_dim_accepts_lights_and_dimmer() {
    assert!(can_dim(&DeviceKind::Bulb).is_ok());
    assert!(can_dim(&DeviceKind::Dimmer).is_ok());
    assert!(can_dim(&DeviceKind::LightStrip).is_ok());
    assert!(can_dim(&DeviceKind::Plug).is_err());
    assert!(can_dim(&DeviceKind::Strip).is_err());
    assert!(can_dim(&DeviceKind::Unknown("IOT.X".into())).is_err());
}

#[test]
fn can_dim_error_names_the_command() {
    let err = can_dim(&DeviceKind::Plug).unwrap_err();
    assert!(err.to_string().contains("`dim`"), "{err}");
}

#[rstest]
#[case(crate::devices::can_get_specs as fn(&DeviceKind) -> anyhow::Result<()>, "`specs`")]
#[case(crate::devices::can_get_presets as fn(&DeviceKind) -> anyhow::Result<()>, "`presets`")]
fn bulb_only_guards_accept_bulb_reject_others(
    #[case] guard: fn(&DeviceKind) -> anyhow::Result<()>,
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
fn color_guards_accept_bulbs_and_lightstrips() {
    for guard in [
        can_set_color_temp as fn(&DeviceKind) -> anyhow::Result<()>,
        can_set_color as fn(&DeviceKind) -> anyhow::Result<()>,
    ] {
        assert!(guard(&DeviceKind::Bulb).is_ok());
        assert!(guard(&DeviceKind::LightStrip).is_ok());
        assert!(guard(&DeviceKind::Plug).is_err());
    }
}

#[rstest]
#[case(can_get_schedules as fn(&DeviceKind) -> anyhow::Result<()>, "`schedules`")]
#[case(can_control_led as fn(&DeviceKind) -> anyhow::Result<()>, "`led`")]
#[case(can_get_clock as fn(&DeviceKind) -> anyhow::Result<()>, "`clock`")]
fn relay_device_guards_accept_plug_dimmer_strip(
    #[case] guard: fn(&DeviceKind) -> anyhow::Result<()>,
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
