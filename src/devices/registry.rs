use serde::Deserialize;
use std::sync::OnceLock;

const TOML_SRC: &str = include_str!("../../devices.toml");

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
