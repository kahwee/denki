//! Device capability registry — loaded from `devices.toml` at startup.
//!
//! `devices.toml` is the source of truth for what each device model supports.
//! The file is embedded into the binary at compile time via `include_str!`
//! and parsed once on first access via `OnceLock`.
//!
//! Use [`lookup`] to find a device by model string, then [`hints`] to build
//! the CLI hint list for `denki info` output.

use serde::Deserialize;
use std::sync::OnceLock;

const TOML_SRC: &str = include_str!("../devices.toml");

/// Device kind — used for both sysinfo-based classification at runtime and
/// TOML deserialization in the static registry.
///
/// `Unknown(String)` is intentionally not deserializable from `devices.toml`:
/// the custom `Deserialize` impl rejects unrecognized strings so TOML drift
/// fails at startup rather than silently producing wrong capability lookups.
/// At runtime, `detect_kind` in main.rs may produce `Unknown` for device
/// types not yet known to denki.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceKind {
    Bulb,
    LightStrip,
    Dimmer,
    Strip,
    Plug,
    /// Tapo devices use the KLAP protocol; capability guards are protocol-level,
    /// not kind-level. This variant appears in `devices.toml` but is never
    /// produced by `detect_kind` (Tapo devices are routed before sysinfo parsing).
    Tapo,
    /// Encountered at runtime for device types not yet recognized by denki.
    /// Not deserializable from `devices.toml` — only constructible in code.
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

/// All known device entries from `devices.toml`, parsed once on first call.
///
/// Panics at startup if `devices.toml` is malformed — this is intentional,
/// since the file is embedded at compile time and should never be invalid.
pub fn all() -> &'static [DeviceEntry] {
    static CACHE: OnceLock<Vec<DeviceEntry>> = OnceLock::new();
    CACHE.get_or_init(|| {
        toml::from_str::<DevicesFile>(TOML_SRC)
            .expect("devices.toml is malformed — rebuild the binary")
            .device
    })
}

/// Look up a device entry by model string.
///
/// Strips the country-code suffix before matching, so `"KL135(US)"` finds
/// the `"KL135"` entry. Matching is case-insensitive.
pub fn lookup(model: &str) -> Option<&'static DeviceEntry> {
    let base = model.split('(').next().unwrap_or(model).trim();
    all().iter().find(|d| d.model.eq_ignore_ascii_case(base))
}

/// Generate the CLI hint string for a single feature name.
///
/// Returns `None` for features with no fixed command form (e.g. `"power"`
/// depends on current state and is handled by the caller).
pub fn hint_for(feature: &str, alias: &str) -> Option<String> {
    match feature {
        "dim" => Some(format!("denki dim \"{alias}\" 80")),
        "color_temp" => Some(format!("denki color-temp \"{alias}\" 2700")),
        "color" => Some(format!(
            "denki color \"{alias}\" --hue 120 --saturation 80 --value 100"
        )),
        "energy" => Some(format!("denki energy \"{alias}\"")),
        "schedules" => Some(format!("denki schedules \"{alias}\"")),
        "led" => Some(format!("denki led \"{alias}\" on|off")),
        "clock" => Some(format!("denki clock \"{alias}\"")),
        "outlets" => Some(format!("denki outlets \"{alias}\"")),
        "specs" => Some(format!("denki specs \"{alias}\"")),
        "presets" => Some(format!("denki presets \"{alias}\"")),
        _ => None,
    }
}

/// Build the complete hint list for a device detail view.
///
/// The state-dependent power hint (`denki on` / `denki off`) is prepended
/// automatically; all other hints are derived from `entry.supports`.
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Loading ───────────────────────────────────────────────────────────────

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
        // If any entry had an unrecognized kind string in devices.toml, `all()`
        // would already have panicked during deserialization. This test just
        // confirms the registry loads without error.
        assert!(!all().is_empty());
    }

    // ── Lookup ────────────────────────────────────────────────────────────────

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

    // ── Feature membership ────────────────────────────────────────────────────

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

    // ── Verified flag ─────────────────────────────────────────────────────────

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

    // ── Protocol ──────────────────────────────────────────────────────────────

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

    // ── hints() ───────────────────────────────────────────────────────────────

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

    // ── CLI hint round-trips ───────────────────────────────────────────────────
    // Parse each hint_for() string through Clap to verify the generated args
    // match the actual CLI definitions. This catches flag renames like --sat → --saturation.

    fn parse_hint(hint: &str) -> Result<crate::cli::Cli, clap::Error> {
        use clap::Parser;
        // hint is "denki <subcommand> [args...]" — split naively on whitespace,
        // stripping surrounding quotes from each token.
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
        // hint_for produces "denki led "alias" on|off" which is not parseable as-is;
        // led requires a concrete on or off value, so test both forms directly.
        use clap::Parser;
        for state in ["on", "off"] {
            let args = ["denki", "led", "plug", state];
            assert!(
                crate::cli::Cli::try_parse_from(args).is_ok(),
                "led {state} failed to parse"
            );
        }
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
}
