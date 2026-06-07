use anyhow::Result;
use colored::Colorize;

use crate::bulb;
use crate::creds;
use crate::devices::{self, DeviceKind};
use crate::dimmer;
use crate::display;
use crate::fmt;
use crate::hosts;
use crate::klap;
use crate::ops;
use crate::resolve::{Resolved, require_kasa, resolve, resolve_outlet};
use crate::strip;

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
                ops::tapo_on(&mut s).await?
            } else {
                ops::tapo_off(&mut s).await?
            }
        }
        hosts::Protocol::Kasa => {
            let json = ops::sysinfo(&r.ip).await?;
            kasa_set_power(&r.ip, &devices::detect_kind(&json), on).await?;
        }
    }
    Ok(())
}

pub(crate) async fn resolve_strip_outlet(
    r: &Resolved,
    cmd: &str,
    outlet: u8,
) -> Result<(String, String, bool)> {
    require_kasa(r, cmd)?;
    let json = ops::sysinfo(&r.ip).await?;
    let s = strip::parse(&json)
        .ok_or_else(|| anyhow::anyhow!("{} does not appear to be a power strip", r.ip))?;
    let child = resolve_outlet(&s, outlet)?;
    Ok((child.id.clone(), child.alias.clone(), child.is_on()))
}

fn strip_for_energy_outlet(
    json: &serde_json::Value,
    ip: &str,
    outlet: u8,
) -> Result<(String, String)> {
    let s = strip::parse(json)
        .ok_or_else(|| anyhow::anyhow!("{ip} does not appear to be a power strip"))?;
    if !s.has_energy_monitoring() {
        anyhow::bail!("{} ({}) does not have energy monitoring", s.alias, s.model);
    }
    let child = resolve_outlet(&s, outlet)?;
    Ok((child.id.clone(), child.alias.clone()))
}

pub(crate) async fn kasa_sysinfo(
    host: &str,
    cmd: &str,
) -> Result<(Resolved, serde_json::Value, DeviceKind)> {
    let r = resolve(host).await?;
    require_kasa(&r, cmd)?;
    let json = ops::sysinfo(&r.ip).await?;
    let kind = devices::detect_kind(&json);
    Ok((r, json, kind))
}

async fn energy_realtime_for(ip: &str, kind: &DeviceKind) -> Result<serde_json::Value> {
    match kind {
        DeviceKind::Bulb | DeviceKind::LightStrip => ops::bulb_energy(ip).await,
        _ => ops::device_energy(ip).await,
    }
}

async fn energy_daily_for(
    ip: &str,
    kind: &DeviceKind,
    year: u16,
    mo: u8,
) -> Result<serde_json::Value> {
    match kind {
        DeviceKind::Bulb | DeviceKind::LightStrip => ops::bulb_energy_daily(ip, year, mo).await,
        _ => ops::device_energy_daily(ip, year, mo).await,
    }
}

async fn energy_monthly_for(ip: &str, kind: &DeviceKind, year: u16) -> Result<serde_json::Value> {
    match kind {
        DeviceKind::Bulb | DeviceKind::LightStrip => ops::bulb_energy_monthly(ip, year).await,
        _ => ops::device_energy_monthly(ip, year).await,
    }
}

pub async fn handle_on(host: &str, outlet: Option<u8>) -> Result<()> {
    let r = resolve(host).await?;
    if let Some(outlet_num) = outlet {
        let (child_id, child_alias, _) =
            resolve_strip_outlet(&r, "on <outlet>", outlet_num).await?;
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
        let (child_id, child_alias, _) =
            resolve_strip_outlet(&r, "off <outlet>", outlet_num).await?;
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
        let (child_id, child_alias, was_on) =
            resolve_strip_outlet(&r, "toggle <outlet>", outlet_num).await?;
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

pub async fn handle_dim(host: &str, level: u8) -> Result<()> {
    let (r, json, kind) = kasa_sysinfo(host, "dim").await?;
    devices::can_dim(&kind)?;
    match kind {
        DeviceKind::Bulb => {
            if bulb::parse(&json).is_some_and(|b| !b.light_state.is_on()) {
                ops::bulb_on(&r.ip).await?;
            }
            ops::bulb_set_brightness(&r.ip, level).await?;
        }
        DeviceKind::Dimmer => {
            if level > 0 && dimmer::parse(&json).is_some_and(|d| !d.is_on()) {
                ops::relay_on(&r.ip).await?;
            }
            ops::dimmer_set_brightness(&r.ip, level).await?;
        }
        _ => unreachable!(),
    }
    println!("Brightness -> {level}%");
    Ok(())
}

pub async fn handle_color_temp(host: &str, kelvin: u16) -> Result<()> {
    let (r, json, kind) = kasa_sysinfo(host, "color-temp").await?;
    devices::can_set_color_temp(&kind)?;
    if bulb::parse(&json).is_some_and(|b| !b.light_state.is_on()) {
        ops::bulb_on(&r.ip).await?;
    }
    ops::bulb_set_color_temp(&r.ip, kelvin).await?;
    println!("Color temperature -> {kelvin}K");
    Ok(())
}

pub async fn handle_color(host: &str, hue: u16, saturation: u8, value: u8) -> Result<()> {
    let (r, json, kind) = kasa_sysinfo(host, "color").await?;
    devices::can_set_color(&kind)?;
    if bulb::parse(&json).is_some_and(|b| !b.light_state.is_on()) {
        ops::bulb_on(&r.ip).await?;
    }
    ops::bulb_set_color(&r.ip, hue, saturation, value).await?;
    println!("Color -> hue:{hue} sat:{saturation} val:{value}");
    Ok(())
}

pub async fn handle_energy(host: &str, outlet: Option<u8>) -> Result<()> {
    let (r, json, kind) = kasa_sysinfo(host, "energy").await?;
    if let Some(outlet_num) = outlet {
        let (child_id, child_alias) = strip_for_energy_outlet(&json, &r.ip, outlet_num)?;
        let resp = ops::strip_outlet_energy(&r.ip, &child_id).await?;
        println!("Outlet {} ({})", outlet_num, child_alias.bold());
        display::print_energy_realtime(&resp);
    } else {
        devices::require_energy(&json, &kind)?;
        display::print_energy_realtime(&energy_realtime_for(&r.ip, &kind).await?);
    }
    Ok(())
}

pub async fn handle_energy_daily(
    host: &str,
    month: Option<String>,
    outlet: Option<u8>,
) -> Result<()> {
    let (r, json, kind) = kasa_sysinfo(host, "energy-daily").await?;
    let month_str = month.unwrap_or_else(|| {
        let (y, m) = fmt::current_year_month();
        format!("{y}-{m:02}")
    });
    let (year, mo) = fmt::parse_year_month(&month_str)?;
    if let Some(outlet_num) = outlet {
        let (child_id, child_alias) = strip_for_energy_outlet(&json, &r.ip, outlet_num)?;
        let resp = ops::strip_outlet_energy_daily(&r.ip, &child_id, year, mo).await?;
        println!("Outlet {} ({})", outlet_num, child_alias.bold());
        display::print_energy_daily(&resp, &month_str);
    } else {
        devices::require_energy(&json, &kind)?;
        display::print_energy_daily(&energy_daily_for(&r.ip, &kind, year, mo).await?, &month_str);
    }
    Ok(())
}

pub async fn handle_energy_monthly(
    host: &str,
    year: Option<u16>,
    outlet: Option<u8>,
) -> Result<()> {
    let (r, json, kind) = kasa_sysinfo(host, "energy-monthly").await?;
    let year = year.unwrap_or_else(|| fmt::current_year_month().0);
    if let Some(outlet_num) = outlet {
        let (child_id, child_alias) = strip_for_energy_outlet(&json, &r.ip, outlet_num)?;
        let resp = ops::strip_outlet_energy_monthly(&r.ip, &child_id, year).await?;
        println!("Outlet {} ({})", outlet_num, child_alias.bold());
        display::print_energy_monthly(&resp, year);
    } else {
        devices::require_energy(&json, &kind)?;
        display::print_energy_monthly(&energy_monthly_for(&r.ip, &kind, year).await?, year);
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

    fn ene_strip_json(n: u8) -> serde_json::Value {
        let children: Vec<serde_json::Value> = (0..n)
            .map(|i| json!({"id": format!("ID{i:02}"), "state": 0, "alias": format!("Outlet {}", i + 1)}))
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

    #[test]
    fn strip_for_energy_outlet_succeeds_on_ene_strip() {
        let json = ene_strip_json(3);
        let (id, alias) = strip_for_energy_outlet(&json, "1.2.3.4", 2).unwrap();
        assert_eq!(id, "ID01");
        assert_eq!(alias, "Outlet 2");
    }

    #[test]
    fn strip_for_energy_outlet_fails_on_non_strip_json() {
        let err = strip_for_energy_outlet(&json!({}), "1.2.3.4", 1).unwrap_err();
        assert!(err.to_string().contains("power strip"), "{err}");
    }

    #[test]
    fn strip_for_energy_outlet_fails_without_ene() {
        let json = no_ene_strip_json();
        let err = strip_for_energy_outlet(&json, "1.2.3.4", 1).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("energy monitoring"), "{msg}");
        assert!(msg.contains("KP303"), "{msg}");
    }

    #[test]
    fn strip_for_energy_outlet_fails_on_out_of_range_outlet() {
        let json = ene_strip_json(2);
        let err = strip_for_energy_outlet(&json, "1.2.3.4", 5).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("outlet 5"), "{msg}");
        assert!(msg.contains("2 outlets"), "{msg}");
    }
}
