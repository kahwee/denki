use denki::devices::DeviceKind;
use denki::{bulb, creds, dimmer, display, hosts, klap, ops, plug, strip, tapo, transport};

use anyhow::{bail, Result};
use clap::{Parser, Subcommand, ValueEnum};
use colored::Colorize;
use std::net::IpAddr;

#[derive(Parser)]
#[command(
    name = "denki",
    about = "Control TP-Link Kasa and Tapo devices from the terminal",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Scan the network for all smart devices
    Scan {
        #[arg(short, long, default_value = "5")]
        timeout: u64,
    },

    /// Show detailed info about a device
    Info {
        /// Device name from scan output, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
    },

    /// Turn a device on
    On {
        /// Device name from scan output, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        /// Outlet number, 1-based (strips only)
        #[arg(value_parser = clap::value_parser!(u8).range(1..))]
        outlet: Option<u8>,
    },

    /// Turn a device off
    Off {
        /// Device name from scan output, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        /// Outlet number, 1-based (strips only)
        #[arg(value_parser = clap::value_parser!(u8).range(1..))]
        outlet: Option<u8>,
    },

    /// Toggle a device on/off
    Toggle {
        /// Device name from scan output, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        /// Outlet number, 1-based (strips only)
        #[arg(value_parser = clap::value_parser!(u8).range(1..))]
        outlet: Option<u8>,
    },

    /// Set brightness 0-100 (KL135 bulbs and HS220 dimmers)
    Dim {
        /// Device name from scan output, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        #[arg(value_parser = clap::value_parser!(u8).range(0..=100))]
        level: u8,
    },

    /// Set color temperature in Kelvin 2500-9000 (KL135 bulbs only)
    ColorTemp {
        /// Device name from scan output, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        #[arg(value_parser = clap::value_parser!(u16).range(2500..=9000))]
        kelvin: u16,
    },

    /// Set HSV color (KL135 bulbs only)
    Color {
        /// Device name from scan output, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        /// Hue 0-360°
        #[arg(long, short = 'H', value_parser = clap::value_parser!(u16).range(0..=360))]
        hue: u16,
        /// Saturation 0-100%
        #[arg(long, short = 's', value_parser = clap::value_parser!(u8).range(0..=100))]
        saturation: u8,
        /// Value (brightness) 0-100%
        #[arg(long, short = 'v', value_parser = clap::value_parser!(u8).range(0..=100))]
        value: u8,
    },

    /// Show real-time energy usage
    Energy {
        /// Device name from scan output, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        /// Outlet number, 1-based (strips only)
        #[arg(value_parser = clap::value_parser!(u8).range(1..))]
        outlet: Option<u8>,
    },

    /// Show daily energy usage for a month (YYYY-MM)
    EnergyDaily {
        /// Device name from scan output, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        /// Month in YYYY-MM format (defaults to current month)
        month: Option<String>,
        /// Outlet number, 1-based (strips only)
        #[arg(long, short = 'o', value_parser = clap::value_parser!(u8).range(1..))]
        outlet: Option<u8>,
    },

    /// Show monthly energy usage for a year (bulbs, light strips, and energy-monitoring plugs)
    EnergyMonthly {
        /// Device name from scan output, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        year: Option<u16>,
        /// Outlet number, 1-based (strips only)
        #[arg(long, short = 'o', value_parser = clap::value_parser!(u8).range(1..))]
        outlet: Option<u8>,
    },

    /// Show bulb hardware specs — lumens, wattage, CRI (bulbs only)
    Specs {
        /// Device name from scan output, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
    },

    /// Show saved light presets (bulbs only)
    Presets {
        /// Device name from scan output, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
    },

    /// Show scheduled rules (plugs, dimmers, and power strips)
    Schedules {
        /// Device name from scan output, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
    },

    /// Control the LED indicator (plugs, dimmers, and power strips)
    Led {
        /// Device name from scan output, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        #[arg(value_enum)]
        state: LedAction,
    },

    /// Show device clock (plugs, dimmers, and power strips)
    Clock {
        /// Device name from scan output, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
    },

    /// Rename a device
    Rename {
        /// Device name from scan output, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        name: String,
    },

    /// Reboot a device
    Restart {
        /// Device name from scan output, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
    },

    /// List all outlets on a power strip, showing 1-based outlet numbers, names, and state (strips only)
    Outlets {
        /// Device name from scan output, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
    },

    /// Rename one outlet on a power strip (1-based outlet number)
    OutletRename {
        /// Device name from scan output, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        /// Outlet number, 1-based
        #[arg(value_parser = clap::value_parser!(u8).range(1..))]
        outlet: u8,
        /// New name for the outlet
        name: String,
    },

    /// Save a friendly name for a device IP (e.g. `denki alias "floor lamp" 192.168.7.254`)
    Alias {
        /// Friendly name to save
        name: String,
        /// IP address of the device
        ip: String,
        /// Mark as a Tapo device (uses KLAP protocol on port 80)
        #[arg(long)]
        klap: bool,
    },

    /// Remove a saved device alias
    Unalias {
        /// Friendly name to remove
        name: String,
    },

    /// List all saved device aliases
    Aliases,

    /// Save Tapo account credentials to avoid setting env vars each session
    Login {
        /// Tapo account email address
        email: String,
        /// Tapo account password (omit to be prompted; never pass on command line in scripts)
        password: Option<String>,
    },
}

#[derive(ValueEnum, Clone)]
enum LedAction {
    On,
    Off,
}

/// Classify a device from its sysinfo JSON response.
///
/// Newer devices use `mic_type`; older devices (HS110, HS105, etc.) use `type`.
/// Detection order for plug-type devices matters:
///   1. "Dimmer" in dev_name → Dimmer (HS220)
///   2. `children` array present → Strip (HS300, KP303, KP400)
///   3. Otherwise → Plug
///
/// For bulb-type devices:
///   1. `length` field present → LightStrip (KL430)
///   2. Otherwise → Bulb
fn detect_kind(json: &serde_json::Value) -> DeviceKind {
    let sysinfo = json.pointer("/system/get_sysinfo");
    let type_str = sysinfo
        .and_then(|s| s.get("mic_type").or_else(|| s.get("type")))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let dev_name = sysinfo
        .and_then(|s| s.get("dev_name"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let has_children = sysinfo.and_then(|s| s.get("children")).is_some();
    let has_length = sysinfo.and_then(|s| s.get("length")).is_some();

    if type_str.contains("SMARTBULB") {
        if has_length {
            DeviceKind::LightStrip
        } else {
            DeviceKind::Bulb
        }
    } else if type_str.contains("PLUG") || type_str.contains("SWITCH") {
        if dev_name.contains("Dimmer") {
            DeviceKind::Dimmer
        } else if has_children {
            DeviceKind::Strip
        } else {
            DeviceKind::Plug
        }
    } else {
        DeviceKind::Unknown(type_str.to_string())
    }
}

/// Open a KLAP session using saved or env-var credentials.
async fn open_tapo(ip: &str) -> Result<klap::KlapSession> {
    let (user, pass) = creds::load()?;
    klap::handshake(ip, &user, &pass).await
}

/// Execute an on/off/toggle action on a Kasa device using an already-fetched sysinfo blob.
///
/// `target_on`:
///   - `Some(true)`  → turn on
///   - `Some(false)` → turn off
///   - `None`        → toggle (reads current state from `json`, no extra network call)
///
/// Returns the new power state (true = on).
async fn kasa_exec_power(
    ip: &str,
    kind: &DeviceKind,
    json: &serde_json::Value,
    target_on: Option<bool>,
) -> Result<bool> {
    can_control_power(kind)?;
    let on = target_on.unwrap_or_else(|| {
        // Determine target by inverting current state — avoids a second sysinfo round trip.
        // Strip: relay_state is absent on HS300 HW 2.0; derive from child outlet states instead.
        match kind {
            DeviceKind::Bulb => json
                .pointer("/system/get_sysinfo/light_state/on_off")
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
                == 0,
            DeviceKind::Strip => {
                !strip::parse(json).map(|s| s.is_any_on()).unwrap_or(false)
            }
            _ => json
                .pointer("/system/get_sysinfo/relay_state")
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
                == 0,
        }
    });
    if matches!(kind, DeviceKind::Bulb) {
        if on {
            ops::bulb_on(ip).await?
        } else {
            ops::bulb_off(ip).await?
        }
    } else {
        if on {
            ops::relay_on(ip).await?
        } else {
            ops::relay_off(ip).await?
        }
    }
    Ok(on)
}

// ── Command compatibility guards ──────────────────────────────────────────────
// Pure synchronous functions: take a DeviceKind, return Ok or a clear error.
// Handlers call these before issuing any network request so the user always
// gets a command-level message ("dim is not supported on plug") rather than
// a raw protocol error from the device.

fn can_control_power(kind: &DeviceKind) -> Result<()> {
    match kind {
        DeviceKind::Bulb | DeviceKind::Plug | DeviceKind::Dimmer | DeviceKind::Strip => Ok(()),
        DeviceKind::LightStrip => anyhow::bail!(
            "light strip power control is not yet implemented \
             (KL430 uses smartlife.iot.lightStrip)"
        ),
        other => anyhow::bail!("{other} does not support power control"),
    }
}

fn can_dim(kind: &DeviceKind) -> Result<()> {
    match kind {
        DeviceKind::Bulb | DeviceKind::Dimmer => Ok(()),
        DeviceKind::LightStrip => anyhow::bail!(
            "`dim` is not yet supported on light strips \
             (KL430 uses smartlife.iot.lightStrip, not smartbulb.lightingservice)"
        ),
        other => anyhow::bail!(
            "`dim` is only supported on KL135-style bulbs and HS220 dimmers, not {other}"
        ),
    }
}

fn can_set_color_temp(kind: &DeviceKind) -> Result<()> {
    match kind {
        DeviceKind::Bulb => Ok(()),
        other => anyhow::bail!(
            "`color-temp` is only supported on KL135-style color bulbs (e.g. KL135), not {other}"
        ),
    }
}

fn can_set_color(kind: &DeviceKind) -> Result<()> {
    match kind {
        DeviceKind::Bulb => Ok(()),
        other => anyhow::bail!(
            "`color` is only supported on KL135-style color bulbs (e.g. KL135), not {other}"
        ),
    }
}

fn can_get_specs(kind: &DeviceKind) -> Result<()> {
    match kind {
        DeviceKind::Bulb => Ok(()),
        other => anyhow::bail!("`specs` is only supported on KL135-style bulbs, not {other}"),
    }
}

fn can_get_presets(kind: &DeviceKind) -> Result<()> {
    match kind {
        DeviceKind::Bulb => Ok(()),
        other => anyhow::bail!("`presets` is only supported on KL135-style bulbs, not {other}"),
    }
}

fn can_get_schedules(kind: &DeviceKind) -> Result<()> {
    match kind {
        DeviceKind::Plug | DeviceKind::Dimmer | DeviceKind::Strip => Ok(()),
        other => anyhow::bail!(
            "`schedules` is only supported on plugs, dimmers, and strips \
             (e.g. KP115, HS220, HS300), not {other}"
        ),
    }
}

fn can_control_led(kind: &DeviceKind) -> Result<()> {
    match kind {
        DeviceKind::Plug | DeviceKind::Dimmer | DeviceKind::Strip => Ok(()),
        other => anyhow::bail!(
            "`led` is only supported on plugs, dimmers, and strips (e.g. KP115, HS220, HS300), not {other}"
        ),
    }
}

fn can_get_clock(kind: &DeviceKind) -> Result<()> {
    match kind {
        DeviceKind::Plug | DeviceKind::Dimmer | DeviceKind::Strip => Ok(()),
        other => anyhow::bail!(
            "`clock` is only supported on plugs, dimmers, and strips \
             (e.g. KP115, HS220, HS300), not {other}"
        ),
    }
}

/// Check that a device supports energy monitoring given its sysinfo and kind.
///
/// Unlike the `can_*` guards above (static / class-level: decided from `DeviceKind`
/// alone), energy support is a runtime / instance-level property: two plugs of the
/// same kind may differ depending on whether the hardware has an ENE chip
/// (KP115/HS110 yes, HS105 no). This check therefore requires both the kind and
/// the live sysinfo blob.
///
/// - Bulb / LightStrip: always supported (smartlife.iot.common.emeter)
/// - Plug: only if it has the ENE feature flag (KP115/HS110 yes, HS105 no)
/// - Dimmer / Strip / Unknown: not supported — bail with a clear message
fn require_energy(json: &serde_json::Value, kind: &DeviceKind) -> Result<()> {
    match kind {
        DeviceKind::Bulb | DeviceKind::LightStrip => Ok(()),
        DeviceKind::Plug => {
            let p = plug::parse(json)
                .ok_or_else(|| anyhow::anyhow!("could not parse plug sysinfo"))?;
            if !p.has_energy_monitoring() {
                anyhow::bail!(
                    "{} ({}) does not have energy monitoring (feature: {:?})",
                    p.alias,
                    p.model,
                    p.feature
                );
            }
            Ok(())
        }
        DeviceKind::Strip => {
            let s = strip::parse(json)
                .ok_or_else(|| anyhow::anyhow!("could not parse strip sysinfo"))?;
            if !s.has_energy_monitoring() {
                anyhow::bail!(
                    "{} ({}) does not have energy monitoring (feature: {:?})",
                    s.alias,
                    s.model,
                    s.feature
                );
            }
            Ok(())
        }
        other => anyhow::bail!("{other} does not support energy monitoring"),
    }
}

/// Returns the current (year, month) using only std — no external date crate.
/// Uses Howard Hinnant's civil_from_days algorithm.
fn current_year_month() -> (u16, u8) {
    use std::time::{SystemTime, UNIX_EPOCH};
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() / 86400)
        .unwrap_or(0) as i64;
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year as u16, m as u8)
}

/// Resolved device: IP + protocol to use.
#[derive(Debug)]
struct Resolved {
    ip: String,
    protocol: hosts::Protocol,
}

/// Resolve a name or IP to (ip, protocol).
/// Resolution order:
///   1. Already an IP → Kasa (default)
///   2. Saved alias in hosts file → uses stored protocol
///   3. Error — no UDP fallback (would block for seconds and miss KLAP devices)
async fn resolve(input: &str) -> Result<Resolved> {
    if input.parse::<IpAddr>().is_ok() {
        return Ok(Resolved {
            ip: input.to_string(),
            protocol: hosts::Protocol::Kasa,
        });
    }
    if let Some(entry) = hosts::lookup(input) {
        println!(
            "{}",
            format!("Using alias \"{input}\" [{}]", entry.ip).dimmed()
        );
        return Ok(Resolved {
            ip: entry.ip,
            protocol: entry.protocol,
        });
    }
    bail!(
        "No device named \"{input}\" found in saved aliases.\n\
         \n\
         If you just ran `denki scan`, use the device IP directly:\n\
         \x20 denki <command> 192.168.x.x\n\
         \n\
         To save a friendly name for next time:\n\
         \x20 denki alias \"<name>\" <ip>"
    )
}

/// Resolve a 1-based outlet number to the matching StripChild, with a clear error if out of range.
fn resolve_outlet(s: &strip::Strip, outlet: u8) -> Result<&strip::StripChild> {
    let idx = (outlet - 1) as usize;
    s.children.get(idx).ok_or_else(|| {
        anyhow::anyhow!(
            "outlet {} does not exist (strip has {} outlets)",
            outlet,
            s.children.len()
        )
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Scan { timeout } => {
            println!("{}", format!("Scanning network for {timeout}s...").dimmed());
            let count = transport::broadcast_each(timeout, |ip, json| match detect_kind(&json) {
                DeviceKind::Bulb => {
                    if let Some(b) = bulb::parse(&json) {
                        display::print_bulb_summary(ip, &b);
                    }
                }
                DeviceKind::LightStrip => {
                    if let Some(b) = bulb::parse(&json) {
                        display::print_lightstrip_summary(ip, &b);
                    }
                }
                DeviceKind::Dimmer => {
                    if let Some(d) = dimmer::parse(&json) {
                        display::print_dimmer_summary(ip, &d);
                    }
                }
                DeviceKind::Strip => {
                    if let Some(s) = strip::parse(&json) {
                        display::print_strip_summary(ip, &s);
                    }
                }
                DeviceKind::Plug => {
                    if let Some(p) = plug::parse(&json) {
                        display::print_plug_summary(ip, &p);
                    }
                }
                // Tapo devices are routed via KLAP before detect_kind is called;
                // Unknown covers any novel Kasa type strings.
                DeviceKind::Tapo => {
                    display::print_unknown_summary(ip, &json, "tapo");
                }
                DeviceKind::Unknown(t) => {
                    display::print_unknown_summary(ip, &json, &t);
                }
            })
            .await?;
            if count == 0 {
                println!("No devices found.");
            } else {
                println!("{}", format!("Found {count} device(s)").dimmed());
            }
        }

        Command::Info { host } => {
            let r = resolve(&host).await?;
            match r.protocol {
                hosts::Protocol::Klap => {
                    let mut session = open_tapo(&r.ip).await?;
                    let json = ops::tapo_device_info(&mut session).await?;
                    match tapo::parse(&json) {
                        Some(d) => display::print_tapo_detail(&r.ip, &d),
                        None => bail!("Could not parse Tapo device info from {}", r.ip),
                    }
                }
                hosts::Protocol::Kasa => {
                    let json = ops::sysinfo(&r.ip).await?;
                    match detect_kind(&json) {
                        DeviceKind::Bulb => match bulb::parse(&json) {
                            Some(b) => display::print_bulb_detail(&r.ip, &b),
                            None => bail!("Could not parse bulb sysinfo from {}", r.ip),
                        },
                        DeviceKind::LightStrip => match bulb::parse(&json) {
                            Some(b) => display::print_lightstrip_detail(&r.ip, &b),
                            None => bail!("Could not parse light strip sysinfo from {}", r.ip),
                        },
                        DeviceKind::Dimmer => match dimmer::parse(&json) {
                            Some(d) => display::print_dimmer_detail(&r.ip, &d),
                            None => bail!("Could not parse dimmer sysinfo from {}", r.ip),
                        },
                        DeviceKind::Strip => match strip::parse(&json) {
                            Some(s) => display::print_strip_detail(&r.ip, &s),
                            None => bail!("Could not parse strip sysinfo from {}", r.ip),
                        },
                        DeviceKind::Plug => match plug::parse(&json) {
                            Some(p) => display::print_plug_detail(&r.ip, &p),
                            None => bail!("Could not parse plug sysinfo from {}", r.ip),
                        },
                        // Tapo devices are routed via KLAP before detect_kind is called;
                        // Unknown covers any novel Kasa type strings.
                        DeviceKind::Tapo => {
                            eprintln!("{}", "Unsupported device type: tapo".yellow());
                            eprintln!("Raw sysinfo from {}:", r.ip);
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&json)
                                    .unwrap_or_else(|_| json.to_string())
                            );
                        }
                        DeviceKind::Unknown(t) => {
                            eprintln!("{}", format!("Unsupported device type: {t}").yellow());
                            eprintln!("Raw sysinfo from {}:", r.ip);
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&json)
                                    .unwrap_or_else(|_| json.to_string())
                            );
                        }
                    }
                }
            }
        }

        Command::On { host, outlet } => {
            let r = resolve(&host).await?;
            if let Some(outlet_num) = outlet {
                if r.protocol != hosts::Protocol::Kasa {
                    bail!("outlet control requires Kasa protocol; save the alias without --klap");
                }
                let json = ops::sysinfo(&r.ip).await?;
                let s = strip::parse(&json)
                    .ok_or_else(|| anyhow::anyhow!("{} does not appear to be a power strip", r.ip))?;
                let child = resolve_outlet(&s, outlet_num)?;
                ops::strip_outlet_on(&r.ip, &child.id).await?;
                println!("Outlet {} ({}) {}", outlet_num, child.alias, "on".green().bold());
            } else {
                match r.protocol {
                    hosts::Protocol::Klap => {
                        let mut s = open_tapo(&r.ip).await?;
                        ops::tapo_on(&mut s).await?;
                    }
                    hosts::Protocol::Kasa => {
                        let json = ops::sysinfo(&r.ip).await?;
                        kasa_exec_power(&r.ip, &detect_kind(&json), &json, Some(true)).await?;
                    }
                }
                println!("{} {}", r.ip, "on".green().bold());
            }
        }

        Command::Off { host, outlet } => {
            let r = resolve(&host).await?;
            if let Some(outlet_num) = outlet {
                if r.protocol != hosts::Protocol::Kasa {
                    bail!("outlet control requires Kasa protocol; save the alias without --klap");
                }
                let json = ops::sysinfo(&r.ip).await?;
                let s = strip::parse(&json)
                    .ok_or_else(|| anyhow::anyhow!("{} does not appear to be a power strip", r.ip))?;
                let child = resolve_outlet(&s, outlet_num)?;
                ops::strip_outlet_off(&r.ip, &child.id).await?;
                println!("Outlet {} ({}) {}", outlet_num, child.alias, "off".dimmed());
            } else {
                match r.protocol {
                    hosts::Protocol::Klap => {
                        let mut s = open_tapo(&r.ip).await?;
                        ops::tapo_off(&mut s).await?;
                    }
                    hosts::Protocol::Kasa => {
                        let json = ops::sysinfo(&r.ip).await?;
                        kasa_exec_power(&r.ip, &detect_kind(&json), &json, Some(false)).await?;
                    }
                }
                println!("{} {}", r.ip, "off".dimmed());
            }
        }

        Command::Toggle { host, outlet } => {
            let r = resolve(&host).await?;
            if let Some(outlet_num) = outlet {
                if r.protocol != hosts::Protocol::Kasa {
                    bail!("outlet control requires Kasa protocol; save the alias without --klap");
                }
                let json = ops::sysinfo(&r.ip).await?;
                let s = strip::parse(&json)
                    .ok_or_else(|| anyhow::anyhow!("{} does not appear to be a power strip", r.ip))?;
                let child = resolve_outlet(&s, outlet_num)?;
                let now_on = if child.is_on() {
                    ops::strip_outlet_off(&r.ip, &child.id).await?;
                    false
                } else {
                    ops::strip_outlet_on(&r.ip, &child.id).await?;
                    true
                };
                let label = if now_on { "on".green().bold() } else { "off".dimmed() };
                println!("Outlet {} ({}) -> {label}", outlet_num, child.alias);
            } else {
                let now_on = match r.protocol {
                    hosts::Protocol::Klap => {
                        let mut s = open_tapo(&r.ip).await?;
                        ops::tapo_toggle(&mut s).await?
                    }
                    hosts::Protocol::Kasa => {
                        let json = ops::sysinfo(&r.ip).await?;
                        kasa_exec_power(&r.ip, &detect_kind(&json), &json, None).await?
                    }
                };
                let label = if now_on { "on".green().bold() } else { "off".dimmed() };
                println!("{} -> {label}", r.ip);
            }
        }

        Command::Dim { host, level } => {
            let r = resolve(&host).await?;
            let json = ops::sysinfo(&r.ip).await?;
            let kind = detect_kind(&json);
            can_dim(&kind)?;
            match kind {
                DeviceKind::Bulb => {
                    // Turn on first if currently off (setting brightness while off has no visible effect)
                    if bulb::parse(&json).is_some_and(|b| !b.light_state.is_on()) {
                        ops::bulb_on(&r.ip).await?;
                    }
                    ops::bulb_set_brightness(&r.ip, level).await?;
                }
                DeviceKind::Dimmer => {
                    // Turn on first if currently off and a non-zero level was requested
                    if level > 0 && dimmer::parse(&json).is_some_and(|d| !d.is_on()) {
                        ops::relay_on(&r.ip).await?;
                    }
                    ops::dimmer_set_brightness(&r.ip, level).await?;
                }
                _ => unreachable!(),
            }
            println!("Brightness -> {level}%");
        }

        Command::ColorTemp { host, kelvin } => {
            let r = resolve(&host).await?;
            let json = ops::sysinfo(&r.ip).await?;
            can_set_color_temp(&detect_kind(&json))?;
            if bulb::parse(&json).is_some_and(|b| !b.light_state.is_on()) {
                ops::bulb_on(&r.ip).await?;
            }
            ops::bulb_set_color_temp(&r.ip, kelvin).await?;
            println!("Color temperature -> {kelvin}K");
        }

        Command::Color {
            host,
            hue,
            saturation,
            value,
        } => {
            let r = resolve(&host).await?;
            let json = ops::sysinfo(&r.ip).await?;
            can_set_color(&detect_kind(&json))?;
            if bulb::parse(&json).is_some_and(|b| !b.light_state.is_on()) {
                ops::bulb_on(&r.ip).await?;
            }
            ops::bulb_set_color(&r.ip, hue, saturation, value).await?;
            println!("Color -> hue:{hue} sat:{saturation} val:{value}");
        }

        Command::Energy { host, outlet } => {
            let r = resolve(&host).await?;
            let json = ops::sysinfo(&r.ip).await?;
            let kind = detect_kind(&json);
            if let Some(outlet_num) = outlet {
                let s = strip::parse(&json)
                    .ok_or_else(|| anyhow::anyhow!("{} does not appear to be a power strip", r.ip))?;
                if !s.has_energy_monitoring() {
                    bail!("{} ({}) does not have energy monitoring", s.alias, s.model);
                }
                let child = resolve_outlet(&s, outlet_num)?;
                let resp = ops::strip_outlet_energy(&r.ip, &child.id).await?;
                println!("Outlet {} ({})", outlet_num, child.alias.bold());
                display::print_energy_realtime(&resp);
            } else {
                require_energy(&json, &kind)?;
                let resp = match &kind {
                    DeviceKind::Bulb | DeviceKind::LightStrip => ops::bulb_energy(&r.ip).await?,
                    _ => ops::device_energy(&r.ip).await?,
                };
                display::print_energy_realtime(&resp);
            }
        }

        Command::EnergyDaily { host, month, outlet } => {
            let host = resolve(&host).await?.ip;
            let month_str = match month {
                Some(m) => m,
                None => {
                    let (y, m) = current_year_month();
                    format!("{y}-{m:02}")
                }
            };
            let parts: Vec<&str> = month_str.split('-').collect();
            if parts.len() != 2 {
                bail!("Month must be in YYYY-MM format");
            }
            let year: u16 = parts[0].parse()?;
            let mo: u8 = parts[1].parse()?;
            if !(1..=12).contains(&mo) {
                bail!("Month must be 01–12, got {mo:02}");
            }

            let json = ops::sysinfo(&host).await?;
            let kind = detect_kind(&json);
            if let Some(outlet_num) = outlet {
                let s = strip::parse(&json)
                    .ok_or_else(|| anyhow::anyhow!("{host} does not appear to be a power strip"))?;
                if !s.has_energy_monitoring() {
                    bail!("{} ({}) does not have energy monitoring", s.alias, s.model);
                }
                let child = resolve_outlet(&s, outlet_num)?;
                let resp = ops::strip_outlet_energy_daily(&host, &child.id, year, mo).await?;
                println!("Outlet {} ({})", outlet_num, child.alias.bold());
                display::print_energy_daily(&resp, &month_str);
            } else {
                require_energy(&json, &kind)?;
                let resp = match &kind {
                    DeviceKind::Bulb | DeviceKind::LightStrip => {
                        ops::bulb_energy_daily(&host, year, mo).await?
                    }
                    _ => ops::device_energy_daily(&host, year, mo).await?,
                };
                display::print_energy_daily(&resp, &month_str);
            }
        }

        Command::EnergyMonthly { host, year, outlet } => {
            let r = resolve(&host).await?;
            let year = year.unwrap_or_else(|| current_year_month().0);
            let json = ops::sysinfo(&r.ip).await?;
            let kind = detect_kind(&json);
            if let Some(outlet_num) = outlet {
                let s = strip::parse(&json)
                    .ok_or_else(|| anyhow::anyhow!("{} does not appear to be a power strip", r.ip))?;
                if !s.has_energy_monitoring() {
                    bail!("{} ({}) does not have energy monitoring", s.alias, s.model);
                }
                let child = resolve_outlet(&s, outlet_num)?;
                let resp = ops::strip_outlet_energy_monthly(&r.ip, &child.id, year).await?;
                println!("Outlet {} ({})", outlet_num, child.alias.bold());
                display::print_energy_monthly(&resp, year);
            } else {
                require_energy(&json, &kind)?;
                let resp = match &kind {
                    DeviceKind::Bulb | DeviceKind::LightStrip => {
                        ops::bulb_energy_monthly(&r.ip, year).await?
                    }
                    _ => ops::device_energy_monthly(&r.ip, year).await?,
                };
                display::print_energy_monthly(&resp, year);
            }
        }

        Command::Specs { host } => {
            let r = resolve(&host).await?;
            let json = ops::sysinfo(&r.ip).await?;
            can_get_specs(&detect_kind(&json))?;
            let resp = ops::bulb_specs(&r.ip).await?;
            display::print_bulb_specs(&resp);
        }

        Command::Presets { host } => {
            let r = resolve(&host).await?;
            let json = ops::sysinfo(&r.ip).await?;
            can_get_presets(&detect_kind(&json))?;
            let resp = ops::bulb_presets(&r.ip).await?;
            display::print_bulb_presets(&resp);
        }

        Command::Schedules { host } => {
            let r = resolve(&host).await?;
            let json = ops::sysinfo(&r.ip).await?;
            can_get_schedules(&detect_kind(&json))?;
            let resp = ops::device_schedules(&r.ip).await?;
            display::print_schedules(&resp);
        }

        Command::Led { host, state } => {
            let r = resolve(&host).await?;
            let json = ops::sysinfo(&r.ip).await?;
            can_control_led(&detect_kind(&json))?;
            let on = matches!(state, LedAction::On);
            ops::device_led(&r.ip, on).await?;
            println!(
                "LED indicator {}",
                if on { "on".green() } else { "off".dimmed() }
            );
        }

        Command::Clock { host } => {
            let r = resolve(&host).await?;
            let json = ops::sysinfo(&r.ip).await?;
            can_get_clock(&detect_kind(&json))?;
            let resp = ops::device_time(&r.ip).await?;
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
            }
        }

        Command::Rename { host, name } => {
            let host = resolve(&host).await?.ip;
            ops::rename(&host, &name).await?;
            println!("Renamed to \"{}\"", name.bold());
        }

        Command::Restart { host } => {
            let host = resolve(&host).await?.ip;
            ops::restart(&host).await?;
            println!("{} rebooting...", host);
        }

        Command::Outlets { host } => {
            let host = resolve(&host).await?.ip;
            let json = ops::sysinfo(&host).await?;
            match strip::parse(&json) {
                Some(s) => display::print_strip_outlets(&s),
                None => bail!("{host} does not appear to be a power strip"),
            }
        }

        Command::OutletRename { host, outlet, name } => {
            let r = resolve(&host).await?;
            let json = ops::sysinfo(&r.ip).await?;
            let s = strip::parse(&json)
                .ok_or_else(|| anyhow::anyhow!("{} does not appear to be a power strip", r.ip))?;
            let child = resolve_outlet(&s, outlet)?;
            let old_name = child.alias.clone();
            ops::strip_outlet_rename(&r.ip, &child.id, &name).await?;
            println!("Outlet {} renamed: {} → {}", outlet, old_name, name.bold());
        }

        Command::Alias { name, ip, klap } => {
            let protocol = if klap {
                hosts::Protocol::Klap
            } else {
                hosts::Protocol::Kasa
            };
            hosts::set(&name, &ip, protocol)?;
            let tag = if klap {
                " (klap)".dimmed()
            } else {
                "".normal()
            };
            println!("Saved: {} → {}{}", name.bold(), ip, tag);
        }

        Command::Unalias { name } => {
            if hosts::remove(&name)? {
                println!("Removed alias \"{}\"", name);
            } else {
                bail!("No alias named \"{name}\" found");
            }
        }

        Command::Aliases => {
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
        }

        Command::Login { email, password } => {
            let password = match password {
                Some(p) => p,
                None => rpassword::prompt_password("Tapo password: ")
                    .map_err(|e| anyhow::anyhow!("Failed to read password: {e}"))?,
            };
            creds::save(&email, &password)?;
            println!("Tapo credentials saved to {}", creds::path_display());
            println!(
                "(File is readable only by you. Use TAPO_USER/TAPO_PASS env vars to override.)"
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detect_kind_separates_bulbs_and_light_strips() {
        let bulb = json!({
            "system": {
                "get_sysinfo": {
                    "mic_type": "IOT.SMARTBULB",
                    "dev_name": "Smart Wi-Fi LED Bulb"
                }
            }
        });
        let strip = json!({
            "system": {
                "get_sysinfo": {
                    "mic_type": "IOT.SMARTBULB",
                    "dev_name": "Smart Wi-Fi Light Strip",
                    "length": 200
                }
            }
        });

        assert_eq!(detect_kind(&bulb).to_string(), "bulb");
        assert_eq!(detect_kind(&strip).to_string(), "light strip");
    }

    #[test]
    fn detect_kind_prefers_dimmer_then_strip_then_plug() {
        let dimmer = json!({
            "system": {
                "get_sysinfo": {
                    "mic_type": "IOT.SMARTPLUGSWITCH",
                    "dev_name": "Smart Wi-Fi Dimmer"
                }
            }
        });
        let strip = json!({
            "system": {
                "get_sysinfo": {
                    "mic_type": "IOT.SMARTPLUGSWITCH",
                    "dev_name": "Smart Wi-Fi Power Strip",
                    "children": []
                }
            }
        });
        let plug = json!({
            "system": {
                "get_sysinfo": {
                    "type": "IOT.SMARTPLUGSWITCH",
                    "dev_name": "Smart Wi-Fi Plug"
                }
            }
        });

        assert_eq!(detect_kind(&dimmer).to_string(), "dimmer");
        assert_eq!(detect_kind(&strip).to_string(), "power strip");
        assert_eq!(detect_kind(&plug).to_string(), "plug");
    }

    #[test]
    fn detect_kind_preserves_unknown_type() {
        let json = json!({
            "system": {
                "get_sysinfo": {
                    "mic_type": "IOT.UNKNOWN"
                }
            }
        });

        let kind = detect_kind(&json);
        assert_eq!(kind.to_string(), "unknown (IOT.UNKNOWN)");
    }

    // ── resolve() tests ───────────────────────────────────────────────────────
    //
    // resolve() is the single entry point for all device targeting. These tests
    // cover the three paths: raw IP (no alias needed), saved alias, and unknown
    // name. The unknown-name case is the most important UX path: after `denki
    // scan` a user sees the device's sysinfo name and may try to use it directly
    // without first running `denki alias`.

    #[tokio::test]
    async fn resolve_raw_ip_returns_kasa_protocol() {
        let r = resolve("192.168.1.1").await.unwrap();
        assert_eq!(r.ip, "192.168.1.1");
        assert!(matches!(r.protocol, hosts::Protocol::Kasa));
    }

    #[tokio::test]
    async fn resolve_unknown_name_error_mentions_ip_and_alias() {
        // Use a name that will never be in any hosts file.
        let err = resolve("ZZZ_no_such_device_99999").await.unwrap_err();
        let msg = err.to_string();
        // Error should tell the user they can use an IP directly — this is the
        // key UX gap: scan shows sysinfo names, not IPs, so the path forward
        // must be explicit.
        assert!(
            msg.contains("192.168.x.x") || msg.contains("IP") || msg.contains("ip"),
            "error should mention using an IP: {msg}"
        );
        assert!(
            msg.contains("alias"),
            "error should mention saving an alias: {msg}"
        );
    }

    #[tokio::test]
    async fn resolve_unknown_name_error_quotes_the_input() {
        let err = resolve("My Nonexistent Lamp").await.unwrap_err();
        assert!(
            err.to_string().contains("My Nonexistent Lamp"),
            "error should quote the unrecognized input so the user knows what failed"
        );
    }

    #[test]
    fn alias_matching_is_case_and_punctuation_insensitive() {
        fn matches(alias: &str, query: &str) -> bool {
            let a = hosts::normalize(alias);
            let q = hosts::normalize(query);
            !q.is_empty() && (a == q || a.contains(&q))
        }
        assert!(matches("Living Room Right Lamp", "living room"));
        assert!(matches("Coat-Rack Lights", "coat rack"));
        assert!(matches("Kitchen Wax Melter", "KITCHEN"));
        assert!(!matches("Back Porch Reading Lamp", "coat rack"));
    }

    // ── Command guard tests ───────────────────────────────────────────────────
    //
    // Each guard function is pure (no I/O), so we test it exhaustively here.
    // The handlers call these before any network request, so these tests cover
    // the routing decisions without needing a live device.

    #[test]
    fn can_control_power_accepts_bulb_plug_dimmer_strip() {
        assert!(can_control_power(&DeviceKind::Bulb).is_ok());
        assert!(can_control_power(&DeviceKind::Plug).is_ok());
        assert!(can_control_power(&DeviceKind::Dimmer).is_ok());
        assert!(can_control_power(&DeviceKind::Strip).is_ok());
        assert!(can_control_power(&DeviceKind::LightStrip).is_err());
        assert!(can_control_power(&DeviceKind::Unknown("IOT.SOMETHING".into())).is_err());
    }

    #[test]
    fn can_dim_accepts_bulb_and_dimmer_only() {
        assert!(can_dim(&DeviceKind::Bulb).is_ok());
        assert!(can_dim(&DeviceKind::Dimmer).is_ok());
        assert!(can_dim(&DeviceKind::LightStrip).is_err());
        assert!(can_dim(&DeviceKind::Plug).is_err());
        assert!(can_dim(&DeviceKind::Strip).is_err());
        assert!(can_dim(&DeviceKind::Unknown("IOT.SOMETHING".into())).is_err());
    }

    #[test]
    fn can_dim_error_names_the_command() {
        let err = can_dim(&DeviceKind::Plug).unwrap_err();
        assert!(
            err.to_string().contains("`dim`"),
            "error should name the command: {err}"
        );
    }

    #[test]
    fn can_dim_lightstrip_error_explains_namespace() {
        let err = can_dim(&DeviceKind::LightStrip).unwrap_err();
        assert!(err.to_string().contains("not yet supported"), "{err}");
        assert!(err.to_string().contains("lightStrip"), "{err}");
    }

    #[test]
    fn can_set_color_temp_accepts_bulb_only() {
        assert!(can_set_color_temp(&DeviceKind::Bulb).is_ok());
        assert!(can_set_color_temp(&DeviceKind::Dimmer).is_err());
        assert!(can_set_color_temp(&DeviceKind::Plug).is_err());
        assert!(can_set_color_temp(&DeviceKind::LightStrip).is_err());
        assert!(can_set_color_temp(&DeviceKind::Strip).is_err());
    }

    #[test]
    fn can_set_color_temp_error_mentions_kl135() {
        let err = can_set_color_temp(&DeviceKind::Plug).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("`color-temp`"), "{msg}");
        assert!(msg.contains("KL135"), "{msg}");
    }

    #[test]
    fn can_set_color_accepts_bulb_only() {
        assert!(can_set_color(&DeviceKind::Bulb).is_ok());
        assert!(can_set_color(&DeviceKind::Dimmer).is_err());
        assert!(can_set_color(&DeviceKind::Plug).is_err());
        assert!(can_set_color(&DeviceKind::LightStrip).is_err());
        assert!(can_set_color(&DeviceKind::Strip).is_err());
    }

    #[test]
    fn can_get_specs_and_presets_accept_bulb_only() {
        for kind in [
            DeviceKind::Dimmer,
            DeviceKind::Plug,
            DeviceKind::Strip,
            DeviceKind::LightStrip,
        ] {
            assert!(can_get_specs(&kind).is_err(), "specs should reject {kind}");
            assert!(
                can_get_presets(&kind).is_err(),
                "presets should reject {kind}"
            );
        }
        assert!(can_get_specs(&DeviceKind::Bulb).is_ok());
        assert!(can_get_presets(&DeviceKind::Bulb).is_ok());
    }

    #[test]
    fn can_get_schedules_accepts_plug_dimmer_strip() {
        assert!(can_get_schedules(&DeviceKind::Plug).is_ok());
        assert!(can_get_schedules(&DeviceKind::Dimmer).is_ok());
        assert!(can_get_schedules(&DeviceKind::Strip).is_ok());
        assert!(can_get_schedules(&DeviceKind::Bulb).is_err());
        assert!(can_get_schedules(&DeviceKind::LightStrip).is_err());
    }

    #[test]
    fn can_get_schedules_error_names_supported_devices() {
        let err = can_get_schedules(&DeviceKind::Bulb).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("`schedules`"), "{msg}");
        assert!(
            msg.contains("KP115") || msg.contains("HS220") || msg.contains("HS300"),
            "{msg}"
        );
    }

    #[test]
    fn can_control_led_accepts_plug_dimmer_and_strip() {
        assert!(can_control_led(&DeviceKind::Plug).is_ok());
        assert!(can_control_led(&DeviceKind::Dimmer).is_ok());
        assert!(can_control_led(&DeviceKind::Strip).is_ok());
        assert!(can_control_led(&DeviceKind::Bulb).is_err());
        assert!(can_control_led(&DeviceKind::LightStrip).is_err());
    }

    #[test]
    fn can_get_clock_accepts_plug_dimmer_strip() {
        assert!(can_get_clock(&DeviceKind::Plug).is_ok());
        assert!(can_get_clock(&DeviceKind::Dimmer).is_ok());
        assert!(can_get_clock(&DeviceKind::Strip).is_ok());
        assert!(can_get_clock(&DeviceKind::Bulb).is_err());
        assert!(can_get_clock(&DeviceKind::LightStrip).is_err());
    }

    #[test]
    fn require_energy_bails_when_plug_parse_fails() {
        // Empty JSON → plug::parse returns None → must error rather than silently pass through
        let empty = serde_json::json!({});
        assert!(
            require_energy(&empty, &DeviceKind::Plug).is_err(),
            "should bail when plug sysinfo cannot be parsed"
        );
    }

    #[test]
    fn require_energy_bails_when_strip_parse_fails() {
        let empty = serde_json::json!({});
        assert!(
            require_energy(&empty, &DeviceKind::Strip).is_err(),
            "should bail when strip sysinfo cannot be parsed"
        );
    }

    #[test]
    fn strip_toggle_uses_child_states_not_relay_state() {
        // HS300 HW 2.0 omits relay_state; is_any_on() must be used instead.
        // Build a minimal strip sysinfo with no relay_state but one outlet on.
        let json = serde_json::json!({
            "system": { "get_sysinfo": {
                "alias": "Test Strip", "model": "HS300(US)",
                "hw_ver": "2.0", "sw_ver": "1.0.0", "rssi": -50,
                "mic_type": "IOT.SMARTPLUGSWITCH",
                "feature": "TIM:ENE",
                "children": [
                    { "id": "8006C2", "alias": "Outlet 1", "state": 1 },
                    { "id": "8006C3", "alias": "Outlet 2", "state": 0 }
                ]
            }}
        });
        let s = strip::parse(&json).expect("should parse");
        assert_eq!(s.relay_state, 0, "relay_state absent → deserialized as 0");
        // relay_state is 0 (absent/defaulted) but outlet 1 is on:
        // is_any_on() must return true; relay_state alone would give the wrong answer
        assert!(s.is_any_on(), "is_any_on should be true when any child state == 1");
        // toggle target = !is_any_on() = false → turn off
        assert!(!s.is_any_on() == false);
    }

    // ── CLI parsing tests ─────────────────────────────────────────────────────

    #[test]
    fn on_off_toggle_parse_as_top_level_commands() {
        let on = Cli::try_parse_from(["denki", "on", "desk lamp"]).unwrap();
        assert!(matches!(on.command, Command::On { ref host, .. } if host == "desk lamp"));

        let off = Cli::try_parse_from(["denki", "off", "desk lamp"]).unwrap();
        assert!(matches!(off.command, Command::Off { ref host, .. } if host == "desk lamp"));

        let tog = Cli::try_parse_from(["denki", "toggle", "desk lamp"]).unwrap();
        assert!(matches!(tog.command, Command::Toggle { ref host, .. } if host == "desk lamp"));
    }

    #[test]
    fn power_subcommand_no_longer_exists() {
        // `power` was replaced by `on`/`off`/`toggle` — clap should reject it
        assert!(Cli::try_parse_from(["denki", "power", "desk lamp", "on"]).is_err());
    }

    #[test]
    fn dim_command_parses_host_and_level() {
        let cli = Cli::try_parse_from(["denki", "dim", "desk lamp", "75"]).unwrap();
        assert!(
            matches!(cli.command, Command::Dim { host, level } if host == "desk lamp" && level == 75)
        );
    }

    #[test]
    fn dim_rejects_level_above_100() {
        assert!(Cli::try_parse_from(["denki", "dim", "desk lamp", "101"]).is_err());
    }

    #[test]
    fn on_with_outlet_parses_host_and_outlet() {
        let cli = Cli::try_parse_from(["denki", "on", "strip", "2"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::On { ref host, outlet: Some(2) } if host == "strip"
        ));
    }

    #[test]
    fn on_without_outlet_parses_host_only() {
        let cli = Cli::try_parse_from(["denki", "on", "strip"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::On { ref host, outlet: None } if host == "strip"
        ));
    }

    #[test]
    fn on_rejects_zero_outlet() {
        assert!(Cli::try_parse_from(["denki", "on", "strip", "0"]).is_err());
    }

    #[test]
    fn off_with_outlet_parses_host_and_outlet() {
        let cli = Cli::try_parse_from(["denki", "off", "strip", "3"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Off { ref host, outlet: Some(3) } if host == "strip"
        ));
    }

    #[test]
    fn off_rejects_zero_outlet() {
        assert!(Cli::try_parse_from(["denki", "off", "strip", "0"]).is_err());
    }

    #[test]
    fn toggle_with_outlet_parses_host_and_outlet() {
        let cli = Cli::try_parse_from(["denki", "toggle", "strip", "2"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Toggle { ref host, outlet: Some(2) } if host == "strip"
        ));
    }

    #[test]
    fn energy_with_outlet_parses_host_and_outlet() {
        let cli = Cli::try_parse_from(["denki", "energy", "strip", "1"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Energy { ref host, outlet: Some(1) } if host == "strip"
        ));
    }

    #[test]
    fn energy_without_outlet_parses_host_only() {
        let cli = Cli::try_parse_from(["denki", "energy", "strip"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Energy { ref host, outlet: None } if host == "strip"
        ));
    }

    #[test]
    fn energy_daily_with_outlet_flag() {
        let cli =
            Cli::try_parse_from(["denki", "energy-daily", "strip", "--outlet", "2"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::EnergyDaily { ref host, outlet: Some(2), .. } if host == "strip"
        ));
    }

    #[test]
    fn energy_monthly_with_outlet_flag() {
        let cli =
            Cli::try_parse_from(["denki", "energy-monthly", "strip", "--outlet", "1"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::EnergyMonthly { ref host, outlet: Some(1), .. } if host == "strip"
        ));
    }

    // ── month validation ──────────────────────────────────────────────────────

    #[test]
    fn energy_daily_rejects_month_zero() {
        // mo=0 is invalid; must fail before any network I/O
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async {
                // Simulate the month parsing path directly
                let month_str = "2025-00".to_string();
                let parts: Vec<&str> = month_str.split('-').collect();
                let mo: u8 = parts[1].parse().unwrap();
                if !(1..=12).contains(&mo) {
                    anyhow::bail!("Month must be 01–12, got {mo:02}");
                }
                Ok::<(), anyhow::Error>(())
            });
        assert!(result.is_err());
    }

    #[test]
    fn energy_daily_rejects_month_13() {
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async {
                let month_str = "2025-13".to_string();
                let parts: Vec<&str> = month_str.split('-').collect();
                let mo: u8 = parts[1].parse().unwrap();
                if !(1..=12).contains(&mo) {
                    anyhow::bail!("Month must be 01–12, got {mo:02}");
                }
                Ok::<(), anyhow::Error>(())
            });
        assert!(result.is_err());
    }

}

// ── devices.toml capability tests ────────────────────────────────────────────
//
// devices.toml is the source of truth for what each device supports.
// These tests go through devices::all() — the same production path used at
// runtime — and verify it matches the can_* guards in both directions:
//
//   1. Every feature listed in devices.toml must be permitted by the guard.
//   2. Every guarded feature NOT listed must be denied by the guard.

#[cfg(test)]
mod capability_tests {
    use super::*;
    use denki::devices;

    // `DeviceEntry::kind` is now typed as `DeviceKind`, so no string parsing is needed.
    // Tapo devices skip kind-level guards (their capability checks are protocol-level).
    fn guard_kind(kind: &DeviceKind) -> Option<&DeviceKind> {
        match kind {
            DeviceKind::Tapo => None,
            k => Some(k),
        }
    }

    const GUARDED: &[&str] = &[
        "power",
        "dim",
        "color_temp",
        "color",
        "specs",
        "presets",
        "schedules",
        "led",
        "clock",
    ];

    fn check(kind: &DeviceKind, feature: &str) -> anyhow::Result<()> {
        match feature {
            "power" => can_control_power(kind),
            "dim" => can_dim(kind),
            "color_temp" => can_set_color_temp(kind),
            "color" => can_set_color(kind),
            "specs" => can_get_specs(kind),
            "presets" => can_get_presets(kind),
            "schedules" => can_get_schedules(kind),
            "led" => can_control_led(kind),
            "clock" => can_get_clock(kind),
            "energy" | "outlets" => Ok(()), // runtime-checked, no static guard
            other => panic!(
                "devices.toml: unknown feature '{other}' — add it to check() or explain \
                 why it has no guard"
            ),
        }
    }

    /// Every (model, feature) in devices.toml must pass the corresponding guard.
    #[test]
    fn listed_features_are_permitted_by_guards() {
        for dev in devices::all() {
            let Some(kind) = guard_kind(&dev.kind) else {
                continue;
            };
            for feature in &dev.supports {
                let result = check(&kind, feature);
                assert!(
                    result.is_ok(),
                    "devices.toml: {} ({}) lists '{}' but the guard rejects it: {}",
                    dev.model,
                    dev.kind,
                    feature,
                    result.unwrap_err(),
                );
            }
        }
    }

    /// Every guarded feature NOT listed for a device must be denied by the guard.
    #[test]
    fn unlisted_guarded_features_are_denied() {
        for dev in devices::all() {
            let Some(kind) = guard_kind(&dev.kind) else {
                continue;
            };
            for &feature in GUARDED {
                if dev.supports.iter().any(|f| f == feature) {
                    continue;
                }
                let result = check(&kind, feature);
                assert!(
                    result.is_err(),
                    "devices.toml: {} ({}) does NOT list '{}' but the guard permits it — \
                     add it to 'supports' or tighten the guard",
                    dev.model,
                    dev.kind,
                    feature,
                );
            }
        }
    }
}
