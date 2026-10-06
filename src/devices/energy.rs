use super::registry::DeviceKind;
use anyhow::Result;

// Energy support is a runtime/instance property, not static/kind-level:
// KP115 has ENE, HS105 does not — both are DeviceKind::Plug.
pub fn require_energy(json: &serde_json::Value, kind: &DeviceKind) -> Result<()> {
    match kind {
        DeviceKind::Bulb | DeviceKind::LightStrip => Ok(()),
        DeviceKind::Plug => {
            let p = crate::plug::parse(json)
                .ok_or_else(|| anyhow::anyhow!("could not parse plug sysinfo"))?;
            if !p.has_energy_monitoring() {
                return Err(crate::error::error(
                    "unsupported_operation",
                    format!(
                        "{} ({}) does not have energy monitoring (feature: {:?})",
                        p.alias, p.model, p.feature
                    ),
                ));
            }
            Ok(())
        }
        DeviceKind::Strip => {
            let s = crate::strip::parse(json)
                .ok_or_else(|| anyhow::anyhow!("could not parse strip sysinfo"))?;
            if !s.has_energy_monitoring() {
                return Err(crate::error::error(
                    "unsupported_operation",
                    format!(
                        "{} ({}) does not have energy monitoring (feature: {:?})",
                        s.alias, s.model, s.feature
                    ),
                ));
            }
            Ok(())
        }
        other => Err(crate::error::error(
            "unsupported_operation",
            format!("{other} does not support energy monitoring"),
        )),
    }
}
