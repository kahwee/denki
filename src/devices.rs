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
        "color" => Some(format!("denki color \"{alias}\" -H 120 -s 80 -v 100")),
        "energy" => Some(format!("denki energy \"{alias}\"")),
        "effects" => Some(format!("denki effects \"{alias}\"")),
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
        other => anyhow::bail!("`dim` is only supported on bulbs and HS220 dimmers, not {other}"),
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

pub fn can_get_effects(kind: &DeviceKind) -> Result<()> {
    match kind {
        DeviceKind::LightStrip => Ok(()),
        other => anyhow::bail!("`effects` is only supported on light strips, not {other}"),
    }
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
mod tests;
