use crate::devices::{DeviceKind, detect_kind};
use serde_json::json;

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
