use anyhow::Result;
use colored::Colorize;

use crate::devices;
use crate::display;
use crate::fmt;
use crate::ops;

use super::shared::KasaContext;

async fn energy_realtime_for(ctx: &KasaContext) -> Result<serde_json::Value> {
    match ctx.kind() {
        crate::devices::DeviceKind::Bulb | crate::devices::DeviceKind::LightStrip => {
            ops::bulb_energy(ctx.ip()).await
        }
        _ => ops::device_energy(ctx.ip()).await,
    }
}

async fn energy_daily_for(ctx: &KasaContext, year: u16, mo: u8) -> Result<serde_json::Value> {
    match ctx.kind() {
        crate::devices::DeviceKind::Bulb | crate::devices::DeviceKind::LightStrip => {
            ops::bulb_energy_daily(ctx.ip(), year, mo).await
        }
        _ => ops::device_energy_daily(ctx.ip(), year, mo).await,
    }
}

async fn energy_monthly_for(ctx: &KasaContext, year: u16) -> Result<serde_json::Value> {
    match ctx.kind() {
        crate::devices::DeviceKind::Bulb | crate::devices::DeviceKind::LightStrip => {
            ops::bulb_energy_monthly(ctx.ip(), year).await
        }
        _ => ops::device_energy_monthly(ctx.ip(), year).await,
    }
}

pub async fn handle_energy(host: &str, outlet: Option<u8>) -> Result<()> {
    let ctx = KasaContext::load(host, "energy").await?;
    if let Some(outlet_num) = outlet {
        let (child_id, child_alias) = ctx.strip_energy_outlet(outlet_num)?;
        let resp = ops::strip_outlet_energy(ctx.ip(), &child_id).await?;
        println!("Outlet {} ({})", outlet_num, child_alias.bold());
        display::print_energy_realtime(&resp);
    } else {
        devices::require_energy(ctx.json(), ctx.kind())?;
        display::print_energy_realtime(&energy_realtime_for(&ctx).await?);
    }
    Ok(())
}

pub async fn handle_energy_daily(
    host: &str,
    month: Option<String>,
    outlet: Option<u8>,
) -> Result<()> {
    let ctx = KasaContext::load(host, "energy-daily").await?;
    let month_str = month.unwrap_or_else(|| {
        let (y, m) = fmt::current_year_month();
        format!("{y}-{m:02}")
    });
    let (year, mo) = fmt::parse_year_month(&month_str)?;
    if let Some(outlet_num) = outlet {
        let (child_id, child_alias) = ctx.strip_energy_outlet(outlet_num)?;
        let resp = ops::strip_outlet_energy_daily(ctx.ip(), &child_id, year, mo).await?;
        println!("Outlet {} ({})", outlet_num, child_alias.bold());
        display::print_energy_daily(&resp, &month_str);
    } else {
        devices::require_energy(ctx.json(), ctx.kind())?;
        display::print_energy_daily(&energy_daily_for(&ctx, year, mo).await?, &month_str);
    }
    Ok(())
}

pub async fn handle_energy_monthly(
    host: &str,
    year: Option<u16>,
    outlet: Option<u8>,
) -> Result<()> {
    let ctx = KasaContext::load(host, "energy-monthly").await?;
    let year = year.unwrap_or_else(|| fmt::current_year_month().0);
    if let Some(outlet_num) = outlet {
        let (child_id, child_alias) = ctx.strip_energy_outlet(outlet_num)?;
        let resp = ops::strip_outlet_energy_monthly(ctx.ip(), &child_id, year).await?;
        println!("Outlet {} ({})", outlet_num, child_alias.bold());
        display::print_energy_monthly(&resp, year);
    } else {
        devices::require_energy(ctx.json(), ctx.kind())?;
        display::print_energy_monthly(&energy_monthly_for(&ctx, year).await?, year);
    }
    Ok(())
}
