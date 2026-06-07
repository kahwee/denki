use anyhow::Result;
use colored::Colorize;

use crate::creds;
use crate::devices::{self, DeviceKind};
use crate::hosts;
use crate::klap;
use crate::ops;
use crate::resolve::{Resolved, resolve};
use crate::strip;

use super::shared::KasaContext;

async fn tapo_session(ip: &str) -> Result<klap::KlapSession> {
    let (user, pass) = creds::load()?;
    klap::handshake(ip, &user, &pass).await
}

fn toggle_target(kind: &DeviceKind, json: &serde_json::Value) -> bool {
    match kind {
        DeviceKind::Bulb => {
            json.pointer("/system/get_sysinfo/light_state/on_off")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
                == 0
        }
        DeviceKind::Strip => !strip::parse(json).is_some_and(|s| s.is_any_on()),
        _ => {
            json.pointer("/system/get_sysinfo/relay_state")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
                == 0
        }
    }
}

async fn kasa_set_power(ip: &str, kind: &DeviceKind, on: bool) -> Result<()> {
    devices::can_control_power(kind)?;
    match (kind, on) {
        (DeviceKind::Bulb, true) => ops::bulb_on(ip).await?,
        (DeviceKind::Bulb, false) => ops::bulb_off(ip).await?,
        (_, true) => ops::relay_on(ip).await?,
        (_, false) => ops::relay_off(ip).await?,
    }
    Ok(())
}

async fn set_device_power(r: &Resolved, on: bool) -> Result<()> {
    match r.protocol {
        hosts::Protocol::Klap => {
            let mut s = tapo_session(&r.ip).await?;
            if on {
                ops::tapo_on(&mut s).await?;
            } else {
                ops::tapo_off(&mut s).await?;
            }
        }
        hosts::Protocol::Kasa => {
            let json = ops::sysinfo(&r.ip).await?;
            kasa_set_power(&r.ip, &devices::detect_kind(&json), on).await?;
        }
    }
    Ok(())
}

pub async fn handle_on(host: &str, outlet: Option<u8>) -> Result<()> {
    let r = resolve(host).await?;
    if let Some(outlet_num) = outlet {
        let ctx = KasaContext::from_resolved(&r, "on <outlet>").await?;
        let (child_id, child_alias, _) = ctx.strip_outlet(outlet_num)?;
        ops::strip_outlet_on(&r.ip, &child_id).await?;
        println!(
            "Outlet {} ({}) {}",
            outlet_num,
            child_alias,
            "on".green().bold()
        );
    } else {
        set_device_power(&r, true).await?;
        println!("{} {}", r.ip, "on".green().bold());
    }
    Ok(())
}

pub async fn handle_off(host: &str, outlet: Option<u8>) -> Result<()> {
    let r = resolve(host).await?;
    if let Some(outlet_num) = outlet {
        let ctx = KasaContext::from_resolved(&r, "off <outlet>").await?;
        let (child_id, child_alias, _) = ctx.strip_outlet(outlet_num)?;
        ops::strip_outlet_off(&r.ip, &child_id).await?;
        println!("Outlet {} ({}) {}", outlet_num, child_alias, "off".dimmed());
    } else {
        set_device_power(&r, false).await?;
        println!("{} {}", r.ip, "off".dimmed());
    }
    Ok(())
}

pub async fn handle_toggle(host: &str, outlet: Option<u8>) -> Result<()> {
    let r = resolve(host).await?;
    if let Some(outlet_num) = outlet {
        let ctx = KasaContext::from_resolved(&r, "toggle <outlet>").await?;
        let (child_id, child_alias, was_on) = ctx.strip_outlet(outlet_num)?;
        let now_on = if was_on {
            ops::strip_outlet_off(&r.ip, &child_id).await?;
            false
        } else {
            ops::strip_outlet_on(&r.ip, &child_id).await?;
            true
        };
        let label = if now_on {
            "on".green().bold()
        } else {
            "off".dimmed()
        };
        println!("Outlet {outlet_num} ({child_alias}) -> {label}");
    } else {
        let now_on = match r.protocol {
            hosts::Protocol::Klap => {
                let mut s = tapo_session(&r.ip).await?;
                ops::tapo_toggle(&mut s).await?
            }
            hosts::Protocol::Kasa => {
                let json = ops::sysinfo(&r.ip).await?;
                let kind = devices::detect_kind(&json);
                let on = toggle_target(&kind, &json);
                kasa_set_power(&r.ip, &kind, on).await?;
                on
            }
        };
        let label = if now_on {
            "on".green().bold()
        } else {
            "off".dimmed()
        };
        println!("{} -> {label}", r.ip);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn toggle_target_bulb_on_returns_false() {
        let json = json!({"system": {"get_sysinfo": {"light_state": {"on_off": 1}}}});
        assert!(!toggle_target(&DeviceKind::Bulb, &json));
    }

    #[test]
    fn toggle_target_bulb_off_returns_true() {
        let json = json!({"system": {"get_sysinfo": {"light_state": {"on_off": 0}}}});
        assert!(toggle_target(&DeviceKind::Bulb, &json));
    }

    #[test]
    fn toggle_target_plug_on_returns_false() {
        let json = json!({"system": {"get_sysinfo": {"relay_state": 1}}});
        assert!(!toggle_target(&DeviceKind::Plug, &json));
    }

    #[test]
    fn toggle_target_plug_off_returns_true() {
        let json = json!({"system": {"get_sysinfo": {"relay_state": 0}}});
        assert!(toggle_target(&DeviceKind::Plug, &json));
    }

    #[test]
    fn toggle_target_dimmer_on_returns_false() {
        let json = json!({"system": {"get_sysinfo": {"relay_state": 1}}});
        assert!(!toggle_target(&DeviceKind::Dimmer, &json));
    }

    #[test]
    fn toggle_target_dimmer_off_returns_true() {
        let json = json!({"system": {"get_sysinfo": {"relay_state": 0}}});
        assert!(toggle_target(&DeviceKind::Dimmer, &json));
    }

    #[test]
    fn toggle_target_strip_any_on_returns_false() {
        let json = json!({
            "system": {"get_sysinfo": {
                "alias": "Strip", "model": "HS300(US)",
                "hw_ver": "1.0", "sw_ver": "1.0.0", "rssi": -40, "feature": "TIM",
                "children": [{"id": "A1", "alias": "Outlet 1", "state": 1},
                             {"id": "A2", "alias": "Outlet 2", "state": 0}]
            }}
        });
        assert!(!toggle_target(&DeviceKind::Strip, &json));
    }

    #[test]
    fn toggle_target_strip_all_off_returns_true() {
        let json = json!({
            "system": {"get_sysinfo": {
                "alias": "Strip", "model": "HS300(US)",
                "hw_ver": "1.0", "sw_ver": "1.0.0", "rssi": -40, "feature": "TIM",
                "children": [{"id": "A1", "alias": "Outlet 1", "state": 0},
                             {"id": "A2", "alias": "Outlet 2", "state": 0}]
            }}
        });
        assert!(toggle_target(&DeviceKind::Strip, &json));
    }
}
