use anyhow::Result;

/// Gate writes by both the advertised device type and the adapter's registry entry.
pub fn require_tapo_auto_power(model: &str, device_type: &str) -> Result<()> {
    if device_type == "SMART.TAPOPLUG"
        && super::lookup(model).is_some_and(|entry| {
            entry.kind == super::DeviceKind::Tapo
                && entry.tapo_auto_supports.iter().any(|cap| cap == "power")
        })
    {
        return Ok(());
    }
    Err(crate::error::error(
        "unsupported_operation",
        "Power control through --tapo is currently supported only for P125 plugs",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_power_requires_registered_model_and_plug_type() {
        assert!(require_tapo_auto_power("P125(US)", "SMART.TAPOPLUG").is_ok());
        for (model, kind) in [
            ("P125", "SMART.TAPOBULB"),
            ("P110", "SMART.TAPOPLUG"),
            ("P125M", "SMART.TAPOPLUG"),
            ("unknown", "SMART.TAPOPLUG"),
        ] {
            assert!(require_tapo_auto_power(model, kind).is_err());
        }
    }
}
