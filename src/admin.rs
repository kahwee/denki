use anyhow::{Result, bail};
use clap::CommandFactory;
use clap_complete::generate;
use colored::Colorize;

use crate::cli::Cli;
use crate::commands::KasaContext;
use crate::creds;
use crate::devices;
use crate::hosts;
use crate::ops;
use crate::resolve::{require_kasa, resolve};
use crate::strip;

pub async fn handle_schedules(host: &str) -> Result<()> {
    let ctx = KasaContext::load(host, "schedules").await?;
    devices::can_get_schedules(ctx.kind())?;
    crate::display::print_schedules(&ops::device_schedules(ctx.ip()).await?);
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

pub async fn handle_clock(host: &str) -> Result<()> {
    let ctx = KasaContext::load(host, "clock").await?;
    devices::can_get_clock(ctx.kind())?;
    let resp = ops::device_time(ctx.ip()).await?;
    if let Some(t) = resp.pointer("/time/get_time") {
        println!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            t["year"].as_u64().unwrap_or(0),
            t["month"].as_u64().unwrap_or(0),
            t["mday"].as_u64().unwrap_or(0),
            t["hour"].as_u64().unwrap_or(0),
            t["min"].as_u64().unwrap_or(0),
            t["sec"].as_u64().unwrap_or(0),
        );
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
    match strip::parse(ctx.json()) {
        Some(s) => crate::display::print_strip_outlets(&s),
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

pub fn handle_alias(name: &str, ip: &str, klap: bool) -> Result<()> {
    let protocol = if klap {
        hosts::Protocol::Klap
    } else {
        hosts::Protocol::Kasa
    };
    hosts::set(name, ip, protocol)?;
    let tag = if klap {
        " (klap)".dimmed()
    } else {
        "".normal()
    };
    println!("Saved: {} → {}{}", name.bold(), ip, tag);
    Ok(())
}

pub fn handle_unalias(name: &str) -> Result<()> {
    if hosts::remove(name)? {
        println!("Removed alias \"{name}\"");
    } else {
        bail!("No alias named \"{name}\" found");
    }
    Ok(())
}

pub fn handle_aliases() -> Result<()> {
    let list = hosts::list()?;
    if list.is_empty() {
        println!("No saved aliases. Use `denki alias <name> <ip> [--klap]` to add one.");
        println!("File: {}", hosts::path_display());
    } else {
        println!(
            "{:<30} {:<18} {}",
            "Name".bold(),
            "IP".bold(),
            "Protocol".bold()
        );
        println!("{}", "─".repeat(58).dimmed());
        for (name, entry) in &list {
            println!("{:<30} {:<18} {}", name, entry.ip, entry.protocol);
        }
        println!(
            "{}",
            format!("({} aliases in {})", list.len(), hosts::path_display()).dimmed()
        );
    }
    Ok(())
}

pub fn handle_login(email: &str, password: Option<String>) -> Result<()> {
    let password = match password {
        Some(p) => p,
        None => rpassword::prompt_password("Tapo password: ")
            .map_err(|e| anyhow::anyhow!("Failed to read password: {e}"))?,
    };
    creds::save(email, &password)?;
    println!("Tapo credentials saved to {}", creds::path_display());
    println!("(File is readable only by you. Use TAPO_USER/TAPO_PASS env vars to override.)");
    Ok(())
}

pub fn handle_completions(shell: clap_complete::Shell) {
    generate(shell, &mut Cli::command(), "denki", &mut std::io::stdout());
}

pub async fn handle_specs(host: &str) -> Result<()> {
    let ctx = KasaContext::load(host, "specs").await?;
    devices::can_get_specs(ctx.kind())?;
    crate::display::print_bulb_specs(&ops::bulb_specs(ctx.ip()).await?);
    Ok(())
}

pub async fn handle_presets(host: &str) -> Result<()> {
    let ctx = KasaContext::load(host, "presets").await?;
    devices::can_get_presets(ctx.kind())?;
    crate::display::print_bulb_presets(&ops::bulb_presets(ctx.ip()).await?);
    Ok(())
}
