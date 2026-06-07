use anyhow::Result;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hosts;
    use serde_json::json;

    fn ene_strip_json(n: u8) -> serde_json::Value {
        let children: Vec<serde_json::Value> = (0..n)
            .map(|i| {
                json!({
                    "id": format!("ID{i:02}"),
                    "state": 0,
                    "alias": format!("Outlet {}", i + 1)
                })
            })
            .collect();
        json!({
            "system": { "get_sysinfo": {
                "alias": "Test Strip", "model": "HS300(US)",
                "hw_ver": "1.0", "sw_ver": "1.0.0", "rssi": -40,
                "feature": "TIM:ENE", "children": children
            }}
        })
    }

    fn no_ene_strip_json() -> serde_json::Value {
        json!({
            "system": { "get_sysinfo": {
                "alias": "Basic Strip", "model": "KP303(US)",
                "hw_ver": "1.0", "sw_ver": "1.0.0", "rssi": -50,
                "feature": "TIM",
                "children": [{"id": "A1", "state": 0, "alias": "Outlet 1"}]
            }}
        })
    }

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
        let ctx = strip_ctx(ene_strip_json(3));
        let (id, alias, on) = ctx.strip_outlet(2).unwrap();
        assert_eq!(id, "ID01");
        assert_eq!(alias, "Outlet 2");
        assert!(!on);
    }

    #[test]
    fn strip_outlet_fails_on_non_strip_json() {
        let ctx = strip_ctx(json!({}));
        let err = ctx.strip_outlet(1).unwrap_err();
        assert!(err.to_string().contains("power strip"), "{err}");
    }

    #[test]
    fn strip_energy_outlet_fails_without_ene() {
        let ctx = strip_ctx(no_ene_strip_json());
        let err = ctx.strip_energy_outlet(1).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("energy monitoring"), "{msg}");
        assert!(msg.contains("KP303"), "{msg}");
    }

    #[test]
    fn strip_outlet_fails_on_out_of_range_outlet() {
        let ctx = strip_ctx(ene_strip_json(2));
        let err = ctx.strip_outlet(5).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("outlet 5"), "{msg}");
        assert!(msg.contains("2 outlets"), "{msg}");
    }
}
