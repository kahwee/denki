//! Sysinfo for TP-Link power strips (HS300, KP303).
//!
//! Detected by the presence of a `children` array in sysinfo.
//! HS300 HW 2.0 omits relay_state — use is_any_on() instead of relay_state directly.
//! HS300 HW 2.0 also uses short child IDs ("00"–"05") that need deviceId prepended.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Strip {
    pub alias: String,
    pub model: String,
    pub hw_ver: String,
    pub sw_ver: String,
    pub rssi: i32,
    #[serde(default)]
    pub relay_state: u8,
    pub feature: Option<String>,
    #[serde(default)]
    pub children: Vec<StripChild>,
}

#[derive(Debug, Deserialize)]
pub struct StripChild {
    pub id: String,
    pub alias: String,
    pub state: u8,
    #[serde(default)]
    pub on_time: u64,
}

impl Strip {
    pub fn is_any_on(&self) -> bool {
        self.children.iter().any(|c| c.state == 1)
    }

    pub fn has_energy_monitoring(&self) -> bool {
        self.feature.as_deref().is_some_and(|f| f.contains("ENE"))
    }
}

impl StripChild {
    pub fn is_on(&self) -> bool {
        self.state == 1
    }

    pub fn on_time_fmt(&self) -> String {
        crate::fmt::on_time(self.on_time)
    }
}

pub fn parse(json: &serde_json::Value) -> Option<Strip> {
    let sysinfo = json.pointer("/system/get_sysinfo")?;
    sysinfo.get("children")?;
    let mut strip: Strip = serde_json::from_value(sysinfo.clone()).ok()?;

    // Full child IDs are 40+ hex chars. Short IDs ("00"–"05") need the deviceId prepended.
    let needs_prefix = strip.children.iter().any(|c| c.id.len() <= 2);
    if needs_prefix && let Some(device_id) = sysinfo.get("deviceId").and_then(|v| v.as_str()) {
        for child in &mut strip.children {
            if child.id.len() <= 2 {
                child.id = format!("{device_id}{}", child.id);
            }
        }
    }

    Some(strip)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support;

    #[test]
    fn fixture_identity_and_inventory_are_explicitly_synthetic() {
        let value = test_support::hs300_hw2_sysinfo();
        let info = &value["system"]["get_sysinfo"];
        assert_eq!(info["deviceId"], "0000000000000000000000000000000000000001");
        assert_eq!(info["sw_ver"], "0.0.0 Build 000000 Rel.000000");
        assert_eq!(info["alias"], "Synthetic Strip");
        for child in info["children"].as_array().unwrap() {
            assert!(
                child["alias"]
                    .as_str()
                    .unwrap()
                    .starts_with("Synthetic Outlet ")
            );
            assert!(matches!(child["on_time"].as_u64(), Some(0 | 60)));
        }
    }

    #[test]
    fn parse_hs300_hw2_succeeds_without_relay_state() {
        // relay_state is absent on HW 2.0 — parse must not fail
        let s =
            parse(&test_support::hs300_hw2_sysinfo()).expect("should parse HS300 HW 2.0 sysinfo");
        assert_eq!(s.alias, "Synthetic Strip");
        assert_eq!(s.model, "HS300(US)");
        assert_eq!(s.children.len(), 6);
    }

    #[test]
    fn parse_hs300_hw2_expands_short_child_ids() {
        // Short IDs ("00"…"05") must be prefixed with deviceId for per-outlet commands
        let device_id = "0000000000000000000000000000000000000001";
        let s = parse(&test_support::hs300_hw2_sysinfo()).unwrap();
        assert_eq!(s.children[0].id, format!("{device_id}00"));
        assert_eq!(s.children[4].id, format!("{device_id}04"));
        assert_eq!(s.children[5].id, format!("{device_id}05"));
    }

    #[test]
    fn parse_hs300_hw2_outlet_states() {
        let s = parse(&test_support::hs300_hw2_sysinfo()).unwrap();
        assert!(s.children[0].is_on(), "Synthetic outlet 1 should be on");
        assert!(!s.children[1].is_on(), "Synthetic outlet 2 should be off");
        assert!(s.children[4].is_on(), "Synthetic outlet 5 should be on");
        assert!(s.children[5].is_on(), "Synthetic outlet 6 should be on");
    }

    #[test]
    fn parse_hs300_hw2_energy_monitoring_detected() {
        let s = parse(&test_support::hs300_hw2_sysinfo()).unwrap();
        assert!(
            s.has_energy_monitoring(),
            "HS300 with TIM:ENE should report energy monitoring"
        );
    }

    #[test]
    fn parse_returns_none_without_children() {
        let json = serde_json::json!({ "system": { "get_sysinfo": { "alias": "not a strip" } } });
        assert!(parse(&json).is_none());
    }

    #[test]
    fn parse_preserves_full_child_ids_when_already_long() {
        // Older firmware returns full 42-char child IDs — must not double-prefix them
        let full_id = "000000000000000000000000000000000000000100";
        let json = serde_json::json!({
            "system": {
                "get_sysinfo": {
                    "alias": "old strip", "model": "HS300", "hw_ver": "1.0",
                    "sw_ver": "1.0.0", "rssi": -40, "feature": "TIM",
                    "deviceId": "0000000000000000000000000000000000000001",
                    "children": [
                        { "id": full_id, "state": 1, "alias": "Outlet 1", "on_time": 100 }
                    ]
                }
            }
        });
        let s = parse(&json).unwrap();
        assert_eq!(
            s.children[0].id, full_id,
            "full ID must not be prefixed again"
        );
    }
}
