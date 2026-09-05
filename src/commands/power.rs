use crate::creds;
use crate::devices::{self, DeviceKind};
use crate::hosts;
use crate::klap;
use crate::ops;
use crate::resolve::{Resolved, resolve};
use crate::strip;
use anyhow::Result;
use clap::ValueEnum;
use colored::Colorize;

use super::shared::{
    StripOutletTarget, print_outlet_power_state, print_outlet_toggle_state, print_power_state,
    resolve_power_target,
};

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
    let (r, target) = resolve_power_target(host, outlet, "on <outlet>").await?;
    if let Some(outlet_num) = outlet {
        let StripOutletTarget {
            child_id,
            child_alias,
            ..
        } = target.expect("outlet target should be present here");
        ops::strip_outlet_on(&r.ip, &child_id).await?;
        print_outlet_power_state(outlet_num, &child_alias, true);
    } else {
        set_device_power(&r, true).await?;
        print_power_state(&r.ip, true);
    }
    Ok(())
}

pub async fn handle_off(host: &str, outlet: Option<u8>) -> Result<()> {
    let (r, target) = resolve_power_target(host, outlet, "off <outlet>").await?;
    if let Some(outlet_num) = outlet {
        let StripOutletTarget {
            child_id,
            child_alias,
            ..
        } = target.expect("outlet target should be present here");
        ops::strip_outlet_off(&r.ip, &child_id).await?;
        print_outlet_power_state(outlet_num, &child_alias, false);
    } else {
        set_device_power(&r, false).await?;
        print_power_state(&r.ip, false);
    }
    Ok(())
}

pub async fn handle_toggle(host: &str, outlet: Option<u8>) -> Result<()> {
    let (r, target) = resolve_power_target(host, outlet, "toggle <outlet>").await?;
    if let Some(outlet_num) = outlet {
        let StripOutletTarget {
            child_id,
            child_alias,
            was_on,
        } = target.expect("outlet target should be present here");
        let now_on = if was_on {
            ops::strip_outlet_off(&r.ip, &child_id).await?;
            false
        } else {
            ops::strip_outlet_on(&r.ip, &child_id).await?;
            true
        };
        print_outlet_toggle_state(outlet_num, &child_alias, now_on);
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
        print_power_state(&r.ip, now_on);
    }
    Ok(())
}

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum GroupAction {
    /// Turn every matched alias on.
    On,
    /// Turn every matched alias off.
    Off,
    /// Toggle every matched alias.
    Toggle,
}

impl GroupAction {
    fn as_verb(&self) -> &'static str {
        match self {
            Self::On => "on",
            Self::Off => "off",
            Self::Toggle => "toggle",
        }
    }
}

pub async fn handle_group(pattern: &str, action: GroupAction) -> Result<()> {
    let matches = hosts::lookup_many(pattern)?;
    if matches.is_empty() {
        anyhow::bail!(
            "No aliases matched pattern \"{pattern}\".\n\
             Check current aliases with: denki aliases"
        );
    }

    println!(
        "{} {} aliases matching \"{}\":",
        matches.len(),
        action.as_verb(),
        pattern
    );

    for (alias, _) in matches {
        match action {
            GroupAction::On => handle_on(&alias, None).await?,
            GroupAction::Off => handle_off(&alias, None).await?,
            GroupAction::Toggle => handle_toggle(&alias, None).await?,
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support;

    #[test]
    fn toggle_target_bulb_on_returns_false() {
        let json = serde_json::json!({"system": {"get_sysinfo": {"light_state": {"on_off": 1}}}});
        assert!(!toggle_target(&DeviceKind::Bulb, &json));
    }

    #[test]
    fn toggle_target_bulb_off_returns_true() {
        let json = serde_json::json!({"system": {"get_sysinfo": {"light_state": {"on_off": 0}}}});
        assert!(toggle_target(&DeviceKind::Bulb, &json));
    }

    #[test]
    fn toggle_target_plug_on_returns_false() {
        let json = serde_json::json!({"system": {"get_sysinfo": {"relay_state": 1}}});
        assert!(!toggle_target(&DeviceKind::Plug, &json));
    }

    #[test]
    fn toggle_target_plug_off_returns_true() {
        let json = serde_json::json!({"system": {"get_sysinfo": {"relay_state": 0}}});
        assert!(toggle_target(&DeviceKind::Plug, &json));
    }

    #[test]
    fn toggle_target_dimmer_on_returns_false() {
        let json = serde_json::json!({"system": {"get_sysinfo": {"relay_state": 1}}});
        assert!(!toggle_target(&DeviceKind::Dimmer, &json));
    }

    #[test]
    fn toggle_target_dimmer_off_returns_true() {
        let json = serde_json::json!({"system": {"get_sysinfo": {"relay_state": 0}}});
        assert!(toggle_target(&DeviceKind::Dimmer, &json));
    }

    #[test]
    fn toggle_target_strip_any_on_returns_false() {
        let json = test_support::strip_sysinfo(
            "Strip",
            "HS300(US)",
            "TIM",
            vec![
                test_support::strip_child("A1", 1, "Outlet 1", 0),
                test_support::strip_child("A2", 0, "Outlet 2", 0),
            ],
        );
        assert!(!toggle_target(&DeviceKind::Strip, &json));
    }

    #[test]
    fn toggle_target_strip_all_off_returns_true() {
        let json = test_support::strip_sysinfo(
            "Strip",
            "HS300(US)",
            "TIM",
            vec![
                test_support::strip_child("A1", 0, "Outlet 1", 0),
                test_support::strip_child("A2", 0, "Outlet 2", 0),
            ],
        );
        assert!(toggle_target(&DeviceKind::Strip, &json));
    }
}
