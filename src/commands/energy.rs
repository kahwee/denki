use anyhow::Result;
use colored::Colorize;

use crate::devices;
use crate::fmt;
use crate::hosts;
use crate::ops;
use crate::resolve::{require_kasa, resolve};

use super::shared::{KasaContext, tapo_session};

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

pub(crate) enum EnergyReader {
    Kasa {
        ip: String,
        bulb: bool,
        child: Option<String>,
    },
    Tapo(crate::klap::KlapSession),
}
impl EnergyReader {
    pub(crate) async fn connect(host: &str, outlet: Option<u8>) -> Result<Self> {
        let resolved = crate::resolve::resolve_quiet(host)?;
        if resolved.protocol == hosts::Protocol::Klap {
            if outlet.is_some() {
                return Err(crate::error::error(
                    "unsupported_operation",
                    "Tapo outlet-level energy monitoring is not supported",
                ));
            }
            let mut session = tapo_session(&resolved.ip).await?;
            ops::tapo_device_info(&mut session).await?;
            return Ok(Self::Tapo(session));
        }
        let ctx = KasaContext::from_resolved(&resolved, "energy").await?;
        devices::require_energy(ctx.json(), ctx.kind())?;
        let child = outlet
            .map(|n| ctx.strip_energy_outlet(n).map(|(id, _)| id))
            .transpose()?;
        Ok(Self::Kasa {
            ip: resolved.ip,
            bulb: matches!(
                ctx.kind(),
                crate::devices::DeviceKind::Bulb | crate::devices::DeviceKind::LightStrip
            ),
            child,
        })
    }
    pub(crate) async fn sample(&mut self) -> Result<crate::energy::Measurement> {
        match self {
            Self::Tapo(session) => {
                crate::energy::Measurement::tapo(&ops::tapo_energy_usage(session).await?)
            }
            Self::Kasa { ip, bulb, child } => {
                let response = if let Some(child) = child {
                    ops::strip_outlet_energy(ip, child).await?
                } else if *bulb {
                    ops::bulb_energy(ip).await?
                } else {
                    ops::device_energy(ip).await?
                };
                crate::energy::Measurement::kasa(&response)
            }
        }
    }
}

pub async fn handle_energy(host: &str, outlet: Option<u8>) -> Result<()> {
    let measurement = EnergyReader::connect(host, outlet).await?.sample().await?;
    crate::output::record(
        serde_json::json!({"device": host, "outlet": outlet, "source": "device", "measurement": measurement}),
    );
    measurement.print();
    Ok(())
}

pub async fn handle_energy_daily(
    host: &str,
    month: Option<String>,
    outlet: Option<u8>,
) -> Result<()> {
    let resolved = resolve(host).await?;
    require_kasa(&resolved, "energy-daily")?;
    let ctx = KasaContext::from_resolved(&resolved, "energy-daily").await?;
    let month_str = month.unwrap_or_else(|| {
        let (y, m) = fmt::current_year_month();
        format!("{y}-{m:02}")
    });
    let (year, mo) = fmt::parse_year_month(&month_str)?;
    if let Some(outlet_num) = outlet {
        let (child_id, child_alias) = ctx.strip_energy_outlet(outlet_num)?;
        let resp = ops::strip_outlet_energy_daily(ctx.ip(), &child_id, year, mo).await?;
        crate::output::println!("Outlet {} ({})", outlet_num, child_alias.bold());
        print_history(&resp, &month_str, false)?;
    } else {
        devices::require_energy(ctx.json(), ctx.kind())?;
        print_history(&energy_daily_for(&ctx, year, mo).await?, &month_str, false)?;
    }
    Ok(())
}

pub async fn handle_energy_monthly(
    host: &str,
    year: Option<u16>,
    outlet: Option<u8>,
) -> Result<()> {
    let resolved = resolve(host).await?;
    require_kasa(&resolved, "energy-monthly")?;
    let ctx = KasaContext::from_resolved(&resolved, "energy-monthly").await?;
    let year = year.unwrap_or_else(|| fmt::current_year_month().0);
    if let Some(outlet_num) = outlet {
        let (child_id, child_alias) = ctx.strip_energy_outlet(outlet_num)?;
        let resp = ops::strip_outlet_energy_monthly(ctx.ip(), &child_id, year).await?;
        crate::output::println!("Outlet {} ({})", outlet_num, child_alias.bold());
        print_history(&resp, &year.to_string(), true)?;
    } else {
        devices::require_energy(ctx.json(), ctx.kind())?;
        print_history(
            &energy_monthly_for(&ctx, year).await?,
            &year.to_string(),
            true,
        )?;
    }
    Ok(())
}

fn print_history(response: &serde_json::Value, period: &str, monthly: bool) -> Result<()> {
    let rows = crate::energy::history(response, monthly)?;
    let key = if monthly { "month" } else { "day" };
    crate::output::println!("Energy history for {period}:");
    for row in &rows {
        crate::output::println!("  {} {}: {} Wh", key, row[key], row["energy_wh"]);
    }
    crate::output::record(
        serde_json::json!({"period": period, "source": "device", "entries": rows}),
    );
    Ok(())
}
