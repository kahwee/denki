use super::registry::DeviceKind;

// Classify a device from its raw sysinfo JSON.
// mic_type is used on newer devices; `type` on older ones (HS110, HS105, etc.).
// For plug-type devices, detection order matters: Dimmer > Strip > Plug.
pub fn detect_kind(json: &serde_json::Value) -> DeviceKind {
    let sysinfo = json.pointer("/system/get_sysinfo");
    let type_str = sysinfo
        .and_then(|s| s.get("mic_type").or_else(|| s.get("type")))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let dev_name = sysinfo
        .and_then(|s| s.get("dev_name"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let has_children = sysinfo.and_then(|s| s.get("children")).is_some();
    let has_length = sysinfo.and_then(|s| s.get("length")).is_some();

    if type_str.contains("SMARTBULB") {
        if has_length {
            DeviceKind::LightStrip
        } else {
            DeviceKind::Bulb
        }
    } else if type_str.contains("PLUG") || type_str.contains("SWITCH") {
        if dev_name.contains("Dimmer") {
            DeviceKind::Dimmer
        } else if has_children {
            DeviceKind::Strip
        } else {
            DeviceKind::Plug
        }
    } else {
        DeviceKind::Unknown(type_str.to_string())
    }
}

/// Returns true if sysinfo is from a plug or switch device (mic_type or type field).
/// Used by plug::parse and dimmer::parse to reject bulb sysinfo early.
pub fn is_plug_switch(sysinfo: &serde_json::Value) -> bool {
    let type_str = sysinfo
        .get("mic_type")
        .or_else(|| sysinfo.get("type"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    type_str.contains("PLUG") || type_str.contains("SWITCH")
}
