//! Sysinfo types for TP-Link smart power strips (HS300, KP303, KP400).
//!
//! Power strips are detected by the presence of a `children` array in sysinfo.
//! Each child represents one controllable outlet.
//!
//! Whole-strip on/off uses set_relay_state. HS300 HW 2.0 omits the top-level
//! relay_state field; current power state must be derived from child outlet
//! states via is_any_on() rather than reading relay_state directly.
//! Individual outlets are controlled by passing a `children` array in the
//! command with the target outlet's id and desired state.

use serde::Deserialize;

/// Top-level sysinfo for a TP-Link smart power strip.
#[derive(Debug, Deserialize)]
pub struct Strip {
    /// Human-readable device name (alias)
    pub alias: String,
    /// Model string, e.g. "HS300(US)"
    pub model: String,
    /// Hardware revision
    pub hw_ver: String,
    /// Firmware version string
    pub sw_ver: String,
    /// Wi-Fi signal strength in dBm
    pub rssi: i32,
    /// Master relay state: 1 = any outlet on, 0 = all off.
    /// Absent on HS300 HW 2.0+ — state is inferred from children instead.
    #[serde(default)]
    pub relay_state: u8,
    /// Capability flags: "TIM", "TIM:ENE"
    pub feature: Option<String>,
    /// Individual outlet states
    #[serde(default)]
    pub children: Vec<StripChild>,
}

/// One controllable outlet on a power strip.
#[derive(Debug, Deserialize)]
pub struct StripChild {
    /// Outlet identifier used for per-outlet commands
    pub id: String,
    /// Human-readable outlet name (alias)
    pub alias: String,
    /// Relay state: 1 = on, 0 = off
    pub state: u8,
    /// Seconds the outlet has been on since last toggle
    #[serde(default)]
    pub on_time: u64,
}

impl Strip {
    pub fn is_any_on(&self) -> bool {
        self.children.iter().any(|c| c.state == 1)
    }

    pub fn has_energy_monitoring(&self) -> bool {
        self.feature
            .as_deref()
            .map(|f| f.contains("ENE"))
            .unwrap_or(false)
    }
}

impl StripChild {
    pub fn is_on(&self) -> bool {
        self.state == 1
    }

    pub fn on_time_fmt(&self) -> String {
        if self.on_time == 0 {
            "off".to_string()
        } else {
            crate::fmt::duration(self.on_time)
        }
    }
}

/// Parse a strip from a raw sysinfo response JSON.
/// Returns None if the sysinfo doesn't contain a `children` array.
///
/// HS300 HW 2.0+ returns short child IDs ("00", "01", …) rather than the full
/// child_id expected by per-outlet commands. When the short form is detected,
/// the `deviceId` is prepended so outlet commands work correctly on all firmware.
pub fn parse(json: &serde_json::Value) -> Option<Strip> {
    let sysinfo = json.pointer("/system/get_sysinfo")?;
    sysinfo.get("children")?;
    let mut strip: Strip = serde_json::from_value(sysinfo.clone()).ok()?;

    // Full child IDs are 40+ hex chars. Short IDs ("00"–"05") need the deviceId prepended.
    let needs_prefix = strip.children.iter().any(|c| c.id.len() <= 2);
    if needs_prefix {
        if let Some(device_id) = sysinfo.get("deviceId").and_then(|v| v.as_str()) {
            for child in &mut strip.children {
                if child.id.len() <= 2 {
                    child.id = format!("{device_id}{}", child.id);
                }
            }
        }
    }

    Some(strip)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn hs300_hw2_sysinfo() -> serde_json::Value {
        // Matches the actual sysinfo shape of HS300(US) HW 2.0 / FW 1.1.2:
        // - no relay_state field
        // - short child IDs ("00", "01", …) instead of full 40-char IDs
        json!({
            "system": {
                "get_sysinfo": {
                    "sw_ver": "1.1.2 Build 241220 Rel.171333",
                    "hw_ver": "2.0",
                    "model": "HS300(US)",
                    "deviceId": "8006EF1981B177DF7C826D7A58FE7461245891E7",
                    "rssi": -23,
                    "alias": "Garage Strip",
                    "mic_type": "IOT.SMARTPLUGSWITCH",
                    "feature": "TIM:ENE",
                    "children": [
                        { "id": "00", "state": 1, "alias": "Eero",       "on_time": 369908 },
                        { "id": "01", "state": 0, "alias": "Plug 2",     "on_time": 0 },
                        { "id": "02", "state": 0, "alias": "Plug 3",     "on_time": 0 },
                        { "id": "03", "state": 0, "alias": "Plug 4",     "on_time": 0 },
                        { "id": "04", "state": 1, "alias": "Printer",    "on_time": 1696 },
                        { "id": "05", "state": 1, "alias": "Ugreen NAS", "on_time": 1670 }
                    ]
                }
            }
        })
    }

    #[test]
    fn parse_hs300_hw2_succeeds_without_relay_state() {
        // relay_state is absent on HW 2.0 — parse must not fail
        let s = parse(&hs300_hw2_sysinfo()).expect("should parse HS300 HW 2.0 sysinfo");
        assert_eq!(s.alias, "Garage Strip");
        assert_eq!(s.model, "HS300(US)");
        assert_eq!(s.children.len(), 6);
    }

    #[test]
    fn parse_hs300_hw2_expands_short_child_ids() {
        // Short IDs ("00"…"05") must be prefixed with deviceId for per-outlet commands
        let device_id = "8006EF1981B177DF7C826D7A58FE7461245891E7";
        let s = parse(&hs300_hw2_sysinfo()).unwrap();
        assert_eq!(s.children[0].id, format!("{device_id}00"));
        assert_eq!(s.children[4].id, format!("{device_id}04"));
        assert_eq!(s.children[5].id, format!("{device_id}05"));
    }

    #[test]
    fn parse_hs300_hw2_outlet_states() {
        let s = parse(&hs300_hw2_sysinfo()).unwrap();
        assert!(s.children[0].is_on(),  "Eero should be on");
        assert!(!s.children[1].is_on(), "Plug 2 should be off");
        assert!(s.children[4].is_on(),  "Printer should be on");
        assert!(s.children[5].is_on(),  "Ugreen NAS should be on");
    }

    #[test]
    fn parse_hs300_hw2_energy_monitoring_detected() {
        let s = parse(&hs300_hw2_sysinfo()).unwrap();
        assert!(s.has_energy_monitoring(), "HS300 with TIM:ENE should report energy monitoring");
    }

    #[test]
    fn parse_returns_none_without_children() {
        let json = json!({ "system": { "get_sysinfo": { "alias": "not a strip" } } });
        assert!(parse(&json).is_none());
    }

    #[test]
    fn parse_preserves_full_child_ids_when_already_long() {
        // Older firmware returns full 42-char child IDs — must not double-prefix them
        let full_id = "8006EF1981B177DF7C826D7A58FE7461245891E700";
        let json = json!({
            "system": {
                "get_sysinfo": {
                    "alias": "old strip", "model": "HS300", "hw_ver": "1.0",
                    "sw_ver": "1.0.0", "rssi": -40, "feature": "TIM",
                    "deviceId": "8006EF1981B177DF7C826D7A58FE7461245891E7",
                    "children": [
                        { "id": full_id, "state": 1, "alias": "Outlet 1", "on_time": 100 }
                    ]
                }
            }
        });
        let s = parse(&json).unwrap();
        assert_eq!(s.children[0].id, full_id, "full ID must not be prefixed again");
    }
}
