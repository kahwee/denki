use anyhow::Result;
use colored::Colorize;

use crate::devices::{self, DeviceKind};
use crate::ops;
use crate::resolve::{Resolved, require_kasa, resolve, resolve_outlet};
use crate::strip;

pub(crate) struct KasaContext {
    resolved: Resolved,
    json: serde_json::Value,
    kind: DeviceKind,
}

impl KasaContext {
    pub(crate) async fn load(host: &str, cmd: &str) -> Result<Self> {
        let resolved = resolve(host).await?;
        Self::from_resolved(&resolved, cmd).await
    }

    pub(crate) async fn from_resolved(resolved: &Resolved, cmd: &str) -> Result<Self> {
        require_kasa(resolved, cmd)?;
        let json = ops::sysinfo(&resolved.ip).await?;
        let kind = devices::detect_kind(&json);
        Ok(Self {
            resolved: Resolved {
                ip: resolved.ip.clone(),
                protocol: resolved.protocol.clone(),
                saved_name: resolved.saved_name.clone(),
            },
            json,
            kind,
        })
    }

    pub(crate) fn ip(&self) -> &str {
        &self.resolved.ip
    }

    pub(crate) fn kind(&self) -> &DeviceKind {
        &self.kind
    }

    pub(crate) fn json(&self) -> &serde_json::Value {
        &self.json
    }

    pub(crate) fn strip_outlet(&self, outlet: u8) -> Result<(String, String, bool)> {
        let s = strip::parse(&self.json)
            .ok_or_else(|| anyhow::anyhow!("{} does not appear to be a power strip", self.ip()))?;
        let child = resolve_outlet(&s, outlet)?;
        Ok((child.id.clone(), child.alias.clone(), child.is_on()))
    }

    pub(crate) fn strip_energy_outlet(&self, outlet: u8) -> Result<(String, String)> {
        let s = strip::parse(&self.json)
            .ok_or_else(|| anyhow::anyhow!("{} does not appear to be a power strip", self.ip()))?;
        if !s.has_energy_monitoring() {
            anyhow::bail!("{} ({}) does not have energy monitoring", s.alias, s.model);
        }
        let child = resolve_outlet(&s, outlet)?;
        Ok((child.id.clone(), child.alias.clone()))
    }
}

pub(crate) struct StripOutletTarget {
    pub(crate) child_id: String,
    pub(crate) child_alias: String,
    pub(crate) was_on: bool,
}

pub(crate) fn power_state_label(is_on: bool) -> colored::ColoredString {
    if is_on {
        "on".green().bold()
    } else {
        "off".dimmed()
    }
}

pub(crate) fn print_power_state(ip: &str, is_on: bool) {
    println!("{} {}", ip, power_state_label(is_on));
}

pub(crate) fn print_outlet_power_state(outlet: u8, alias: &str, is_on: bool) {
    println!("Outlet {} ({}) {}", outlet, alias, power_state_label(is_on));
}

pub(crate) fn print_outlet_toggle_state(outlet: u8, alias: &str, now_on: bool) {
    println!("Outlet {outlet} ({alias}) -> {}", power_state_label(now_on));
}

pub(crate) async fn resolve_power_target(
    host: &str,
    outlet: Option<u8>,
    cmd: &str,
) -> Result<(Resolved, Option<StripOutletTarget>)> {
    let r = resolve(host).await?;
    let target = if let Some(outlet_num) = outlet {
        let ctx = KasaContext::from_resolved(&r, cmd).await?;
        let (child_id, child_alias, was_on) = ctx.strip_outlet(outlet_num)?;
        Some(StripOutletTarget {
            child_id,
            child_alias,
            was_on,
        })
    } else {
        None
    };
    Ok((r, target))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hosts;
    use crate::test_support;

    fn strip_ctx(json: serde_json::Value) -> KasaContext {
        KasaContext {
            resolved: Resolved {
                ip: "1.2.3.4".to_string(),
                protocol: hosts::Protocol::Kasa,
                saved_name: None,
            },
            kind: DeviceKind::Strip,
            json,
        }
    }

    #[test]
    fn strip_outlet_succeeds_on_ene_strip() {
        let ctx = strip_ctx(test_support::strip_sysinfo(
            "Test Strip",
            "HS300(US)",
            "TIM:ENE",
            vec![
                test_support::strip_child("ID00", 0, "Outlet 1", 0),
                test_support::strip_child("ID01", 0, "Outlet 2", 0),
                test_support::strip_child("ID02", 0, "Outlet 3", 0),
            ],
        ));
        let (id, alias, on) = ctx.strip_outlet(2).unwrap();
        assert_eq!(id, "ID01");
        assert_eq!(alias, "Outlet 2");
        assert!(!on);
    }

    #[test]
    fn strip_outlet_fails_on_non_strip_json() {
        let ctx = strip_ctx(serde_json::json!({}));
        let err = ctx.strip_outlet(1).unwrap_err();
        assert!(err.to_string().contains("power strip"), "{err}");
    }

    #[test]
    fn strip_energy_outlet_fails_without_ene() {
        let ctx = strip_ctx(test_support::strip_sysinfo(
            "Basic Strip",
            "KP303(US)",
            "TIM",
            vec![test_support::strip_child("A1", 0, "Outlet 1", 0)],
        ));
        let err = ctx.strip_energy_outlet(1).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("energy monitoring"), "{msg}");
        assert!(msg.contains("KP303"), "{msg}");
    }

    #[test]
    fn strip_outlet_fails_on_out_of_range_outlet() {
        let ctx = strip_ctx(test_support::strip_sysinfo(
            "Test Strip",
            "HS300(US)",
            "TIM:ENE",
            vec![
                test_support::strip_child("ID00", 0, "Outlet 1", 0),
                test_support::strip_child("ID01", 0, "Outlet 2", 0),
            ],
        ));
        let err = ctx.strip_outlet(5).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("outlet 5"), "{msg}");
        assert!(msg.contains("2 outlets"), "{msg}");
    }
}
