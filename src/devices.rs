//! Device capability registry — loaded from `devices.toml` at compile time.
//! Also owns `detect_kind` and all command capability guards.

use anyhow::Result;
use serde::Deserialize;
use std::sync::OnceLock;

const TOML_SRC: &str = include_str!("../devices.toml");

/// `Unknown(String)` is never deserializable from `devices.toml` — unrecognized
/// kind strings fail at startup rather than silently producing wrong capability lookups.
/// At runtime, `detect_kind` may produce `Unknown` for device types not yet known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceKind {
    Bulb,
    LightStrip,
    Dimmer,
    Strip,
    Plug,
    /// Tapo devices are routed via KLAP before sysinfo parsing; this variant
    /// appears in `devices.toml` but is never produced by `detect_kind`.
    Tapo,
    Unknown(String),
}

impl std::fmt::Display for DeviceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceKind::Bulb => write!(f, "bulb"),
            DeviceKind::LightStrip => write!(f, "light strip"),
            DeviceKind::Dimmer => write!(f, "dimmer"),
            DeviceKind::Strip => write!(f, "power strip"),
            DeviceKind::Plug => write!(f, "plug"),
            DeviceKind::Tapo => write!(f, "tapo"),
            DeviceKind::Unknown(t) => write!(f, "unknown ({t})"),
        }
    }
}

impl<'de> serde::Deserialize<'de> for DeviceKind {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        match s.as_str() {
            "Bulb" => Ok(DeviceKind::Bulb),
            "LightStrip" => Ok(DeviceKind::LightStrip),
            "Dimmer" => Ok(DeviceKind::Dimmer),
            "Strip" => Ok(DeviceKind::Strip),
            "Plug" => Ok(DeviceKind::Plug),
            "Tapo" => Ok(DeviceKind::Tapo),
            other => Err(serde::de::Error::unknown_variant(
                other,
                &["Bulb", "LightStrip", "Dimmer", "Strip", "Plug", "Tapo"],
            )),
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct DeviceEntry {
    pub model: String,
    pub kind: DeviceKind,
    pub verified: bool,
    #[serde(default)]
    pub protocol: Option<String>,
    pub supports: Vec<String>,
}

#[derive(Deserialize)]
struct DevicesFile {
    device: Vec<DeviceEntry>,
}

pub fn all() -> &'static [DeviceEntry] {
    static CACHE: OnceLock<Vec<DeviceEntry>> = OnceLock::new();
    CACHE.get_or_init(|| {
        toml::from_str::<DevicesFile>(TOML_SRC)
            .expect("devices.toml is malformed — rebuild the binary")
            .device
    })
}

/// Strips country-code suffix before matching ("KL135(US)" → "KL135"). Case-insensitive.
pub fn lookup(model: &str) -> Option<&'static DeviceEntry> {
    let base = model.split('(').next().unwrap_or(model).trim();
    all().iter().find(|d| d.model.eq_ignore_ascii_case(base))
}

pub fn hint_for(feature: &str, alias: &str) -> Option<String> {
    match feature {
        "dim" => Some(format!("denki dim \"{alias}\" 80")),
        "color_temp" => Some(format!("denki color-temp \"{alias}\" 2700")),
        "color" => Some(format!(
            "denki color \"{alias}\" --hue 120 --saturation 80 --value 100"
        )),
        "energy" => Some(format!("denki energy \"{alias}\"")),
        "schedules" => Some(format!("denki schedules \"{alias}\"")),
        "led" => Some(format!("denki led \"{alias}\" on")),
        "clock" => Some(format!("denki clock \"{alias}\"")),
        "outlets" => Some(format!("denki outlets \"{alias}\"")),
        "specs" => Some(format!("denki specs \"{alias}\"")),
        "presets" => Some(format!("denki presets \"{alias}\"")),
        _ => None,
    }
}

pub fn hints(entry: &DeviceEntry, alias: &str, is_on: bool) -> Vec<String> {
    let action = if is_on { "off" } else { "on" };
    let mut out = vec![format!("denki {action} \"{alias}\"")];
    for feature in &entry.supports {
        if let Some(h) = hint_for(feature, alias) {
            out.push(h);
        }
    }
    out
}

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

// Command capability guards — pure functions, called before any network I/O.

pub fn can_control_power(kind: &DeviceKind) -> Result<()> {
    match kind {
        DeviceKind::Bulb | DeviceKind::Plug | DeviceKind::Dimmer | DeviceKind::Strip => Ok(()),
        DeviceKind::LightStrip => anyhow::bail!(
            "light strip power control is not yet implemented \
             (KL430 uses smartlife.iot.lightStrip)"
        ),
        other => anyhow::bail!("{other} does not support power control"),
    }
}

pub fn can_dim(kind: &DeviceKind) -> Result<()> {
    match kind {
        DeviceKind::Bulb | DeviceKind::Dimmer => Ok(()),
        DeviceKind::LightStrip => anyhow::bail!(
            "`dim` is not yet supported on light strips \
             (KL430 uses smartlife.iot.lightStrip, not smartbulb.lightingservice)"
        ),
        other => anyhow::bail!(
            "`dim` is only supported on bulbs and HS220 dimmers, not {other}"
        ),
    }
}

fn require_bulb(kind: &DeviceKind, cmd: &str, models: &str) -> Result<()> {
    match kind {
        DeviceKind::Bulb => Ok(()),
        other => anyhow::bail!("`{cmd}` is only supported on {models}, not {other}"),
    }
}

fn require_relay_device(kind: &DeviceKind, cmd: &str) -> Result<()> {
    match kind {
        DeviceKind::Plug | DeviceKind::Dimmer | DeviceKind::Strip => Ok(()),
        other => anyhow::bail!(
            "`{cmd}` is only supported on plugs, dimmers, and strips \
             (e.g. KP115, HS220, HS300), not {other}"
        ),
    }
}

pub fn can_set_color_temp(kind: &DeviceKind) -> Result<()> {
    require_bulb(kind, "color-temp", "color bulbs (e.g. KL135)")
}

pub fn can_set_color(kind: &DeviceKind) -> Result<()> {
    require_bulb(kind, "color", "bulbs")
}

pub fn can_get_specs(kind: &DeviceKind) -> Result<()> {
    require_bulb(kind, "specs", "bulbs")
}

pub fn can_get_presets(kind: &DeviceKind) -> Result<()> {
    require_bulb(kind, "presets", "bulbs")
}

pub fn can_get_schedules(kind: &DeviceKind) -> Result<()> {
    require_relay_device(kind, "schedules")
}

pub fn can_control_led(kind: &DeviceKind) -> Result<()> {
    require_relay_device(kind, "led")
}

pub fn can_get_clock(kind: &DeviceKind) -> Result<()> {
    require_relay_device(kind, "clock")
}

// Energy support is a runtime/instance property, not static/kind-level:
// KP115 has ENE, HS105 does not — both are DeviceKind::Plug.
pub fn require_energy(json: &serde_json::Value, kind: &DeviceKind) -> Result<()> {
    match kind {
        DeviceKind::Bulb | DeviceKind::LightStrip => Ok(()),
        DeviceKind::Plug => {
            let p = crate::plug::parse(json)
                .ok_or_else(|| anyhow::anyhow!("could not parse plug sysinfo"))?;
            if !p.has_energy_monitoring() {
                anyhow::bail!(
                    "{} ({}) does not have energy monitoring (feature: {:?})",
                    p.alias,
                    p.model,
                    p.feature
                );
            }
            Ok(())
        }
        DeviceKind::Strip => {
            let s = crate::strip::parse(json)
                .ok_or_else(|| anyhow::anyhow!("could not parse strip sysinfo"))?;
            if !s.has_energy_monitoring() {
                anyhow::bail!(
                    "{} ({}) does not have energy monitoring (feature: {:?})",
                    s.alias,
                    s.model,
                    s.feature
                );
            }
            Ok(())
        }
        other => anyhow::bail!("{other} does not support energy monitoring"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn all_loads_without_panicking() {
        assert!(!all().is_empty());
    }

    #[test]
    fn all_contains_expected_models() {
        let models: Vec<&str> = all().iter().map(|d| d.model.as_str()).collect();
        for expected in [
            "KL135", "KP115", "HS105", "HS110", "HS220", "HS300", "KL430", "P125",
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

    #[test]
    fn can_set_color_temp_accepts_bulb_only() {
        assert!(can_set_color_temp(&DeviceKind::Bulb).is_ok());
        assert!(can_set_color_temp(&DeviceKind::Dimmer).is_err());
        assert!(can_set_color_temp(&DeviceKind::Plug).is_err());
        assert!(can_set_color_temp(&DeviceKind::LightStrip).is_err());
        assert!(can_set_color_temp(&DeviceKind::Strip).is_err());
    }

    #[test]
    fn can_set_color_temp_error_mentions_kl135() {
        let err = can_set_color_temp(&DeviceKind::Plug).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("`color-temp`"), "{msg}");
        assert!(msg.contains("KL135"), "{msg}");
    }

    #[test]
    fn can_set_color_accepts_bulb_only() {
        assert!(can_set_color(&DeviceKind::Bulb).is_ok());
        assert!(can_set_color(&DeviceKind::Dimmer).is_err());
        assert!(can_set_color(&DeviceKind::Plug).is_err());
        assert!(can_set_color(&DeviceKind::LightStrip).is_err());
        assert!(can_set_color(&DeviceKind::Strip).is_err());
    }

    #[test]
    fn can_get_specs_and_presets_accept_bulb_only() {
        for kind in [
            DeviceKind::Dimmer,
            DeviceKind::Plug,
            DeviceKind::Strip,
            DeviceKind::LightStrip,
        ] {
            assert!(can_get_specs(&kind).is_err(), "specs should reject {kind}");
            assert!(can_get_presets(&kind).is_err(), "presets should reject {kind}");
        }
        assert!(can_get_specs(&DeviceKind::Bulb).is_ok());
        assert!(can_get_presets(&DeviceKind::Bulb).is_ok());
    }

    #[test]
    fn can_get_schedules_accepts_plug_dimmer_strip() {
        assert!(can_get_schedules(&DeviceKind::Plug).is_ok());
        assert!(can_get_schedules(&DeviceKind::Dimmer).is_ok());
        assert!(can_get_schedules(&DeviceKind::Strip).is_ok());
        assert!(can_get_schedules(&DeviceKind::Bulb).is_err());
        assert!(can_get_schedules(&DeviceKind::LightStrip).is_err());
    }

    #[test]
    fn can_get_schedules_error_names_supported_devices() {
        let err = can_get_schedules(&DeviceKind::Bulb).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("`schedules`"), "{msg}");
        assert!(
            msg.contains("KP115") || msg.contains("HS220") || msg.contains("HS300"),
            "{msg}"
        );
    }

    #[test]
    fn can_control_led_accepts_plug_dimmer_and_strip() {
        assert!(can_control_led(&DeviceKind::Plug).is_ok());
        assert!(can_control_led(&DeviceKind::Dimmer).is_ok());
        assert!(can_control_led(&DeviceKind::Strip).is_ok());
        assert!(can_control_led(&DeviceKind::Bulb).is_err());
        assert!(can_control_led(&DeviceKind::LightStrip).is_err());
    }

    #[test]
    fn can_get_clock_accepts_plug_dimmer_strip() {
        assert!(can_get_clock(&DeviceKind::Plug).is_ok());
        assert!(can_get_clock(&DeviceKind::Dimmer).is_ok());
        assert!(can_get_clock(&DeviceKind::Strip).is_ok());
        assert!(can_get_clock(&DeviceKind::Bulb).is_err());
        assert!(can_get_clock(&DeviceKind::LightStrip).is_err());
    }

    #[test]
    fn require_energy_bails_when_plug_parse_fails() {
        assert!(require_energy(&json!({}), &DeviceKind::Plug).is_err());
    }

    #[test]
    fn require_energy_bails_when_strip_parse_fails() {
        assert!(require_energy(&json!({}), &DeviceKind::Strip).is_err());
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
}
