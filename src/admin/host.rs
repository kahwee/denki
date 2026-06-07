use anyhow::{Result, bail};
use colored::Colorize;

use crate::commands::KasaContext;
use crate::devices;
use crate::display;
use crate::ops;
use crate::resolve::{require_kasa, resolve};

pub async fn handle_schedules(host: &str) -> Result<()> {
    let ctx = KasaContext::load(host, "schedules").await?;
    devices::can_get_schedules(ctx.kind())?;
    display::print_schedules(&ops::device_schedules(ctx.ip()).await?);
    Ok(())
}

pub async fn handle_led(host: &str, on: bool) -> Result<()> {
    let ctx = KasaContext::load(host, "led").await?;
    devices::can_control_led(ctx.kind())?;
    ops::device_led(ctx.ip(), on).await?;
    println!(
        "LED indicator {}",
        if on { "on".green() } else { "off".dimmed() }
    );
    Ok(())
}

fn format_clock(resp: &serde_json::Value) -> Option<String> {
    let t = resp.pointer("/time/get_time")?;
    Some(format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        t["year"].as_u64().unwrap_or(0),
        t["month"].as_u64().unwrap_or(0),
        t["mday"].as_u64().unwrap_or(0),
        t["hour"].as_u64().unwrap_or(0),
        t["min"].as_u64().unwrap_or(0),
        t["sec"].as_u64().unwrap_or(0),
    ))
}

pub async fn handle_clock(host: &str) -> Result<()> {
    let ctx = KasaContext::load(host, "clock").await?;
    devices::can_get_clock(ctx.kind())?;
    let resp = ops::device_time(ctx.ip()).await?;
    if let Some(line) = format_clock(&resp) {
        println!("{line}");
    } else {
        bail!("Unexpected response from {}: no time data", ctx.ip());
    }
    Ok(())
}

pub async fn handle_rename(host: &str, name: &str) -> Result<()> {
    let r = resolve(host).await?;
    require_kasa(&r, "rename")?;
    ops::rename(&r.ip, name).await?;
    println!("Renamed to \"{}\"", name.bold());
    Ok(())
}

pub async fn handle_restart(host: &str) -> Result<()> {
    let r = resolve(host).await?;
    require_kasa(&r, "restart")?;
    ops::restart(&r.ip).await?;
    println!("{} rebooting...", r.ip);
    Ok(())
}

pub async fn handle_outlets(host: &str) -> Result<()> {
    let ctx = KasaContext::load(host, "outlets").await?;
    match crate::strip::parse(ctx.json()) {
        Some(s) => display::print_strip_outlets(&s),
        None => bail!("{} does not appear to be a power strip", ctx.ip()),
    }
    Ok(())
}

pub async fn handle_outlet_rename(host: &str, outlet: u8, name: &str) -> Result<()> {
    let ctx = KasaContext::load(host, "outlet-rename").await?;
    let (child_id, child_alias, _) = ctx.strip_outlet(outlet)?;
    ops::strip_outlet_rename(ctx.ip(), &child_id, name).await?;
    println!(
        "Outlet {} renamed: {} → {}",
        outlet,
        child_alias,
        name.bold()
    );
    Ok(())
}

pub async fn handle_specs(host: &str) -> Result<()> {
    let ctx = KasaContext::load(host, "specs").await?;
    devices::can_get_specs(ctx.kind())?;
    display::print_bulb_specs(&ops::bulb_specs(ctx.ip()).await?);
    Ok(())
}

pub async fn handle_presets(host: &str) -> Result<()> {
    let ctx = KasaContext::load(host, "presets").await?;
    devices::can_get_presets(ctx.kind())?;
    display::print_bulb_presets(&ops::bulb_presets(ctx.ip()).await?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support;

    #[test]
    fn format_clock_handles_complete_payload() {
        let resp = test_support::clock_response(2026, 6, 7, 22, 44, 54);
        assert_eq!(format_clock(&resp).as_deref(), Some("2026-06-07 22:44:54"));
    }

    #[test]
    fn format_clock_returns_none_without_time() {
        assert!(format_clock(&serde_json::json!({})).is_none());
    }
}
