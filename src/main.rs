mod bulb;
mod cipher;
mod dimmer;
mod display;
mod klap;
mod ops;
mod plug;
mod strip;
mod tapo;
mod transport;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand, ValueEnum};
use colored::Colorize;

#[derive(Parser)]
#[command(
    name = "denki",
    about = "Control TP-Link smart bulbs and plugs from the terminal",
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
    Info { host: String },

    /// Turn a device on, off, or toggle it
    Power {
        host: String,
        #[arg(value_enum)]
        state: PowerAction,
    },

    /// Set brightness 0-100 (bulbs only)
    Dim {
        host: String,
        #[arg(value_parser = clap::value_parser!(u8).range(0..=100))]
        level: u8,
    },

    /// Set color temperature in Kelvin 2500-9000 (bulbs only)
    Warmth {
        host: String,
        #[arg(value_parser = clap::value_parser!(u16).range(2500..=9000))]
        kelvin: u16,
    },

    /// Set color in HSV — hue 0-360, saturation 0-100, value 0-100 (bulbs only)
    Color {
        host: String,
        #[arg(value_parser = clap::value_parser!(u16).range(0..=360))]
        hue: u16,
        #[arg(value_parser = clap::value_parser!(u8).range(0..=100))]
        saturation: u8,
        #[arg(value_parser = clap::value_parser!(u8).range(0..=100))]
        value: u8,
    },

    /// Show real-time energy usage
    Energy { host: String },

    /// Show daily energy usage for a month (YYYY-MM)
    EnergyDaily {
        host: String,
        /// Month in YYYY-MM format (defaults to current month)
        #[arg(default_value = "2026-05")]
        month: String,
    },

    /// Show monthly energy usage for a year (plugs only)
    EnergyMonthly {
        host: String,
        #[arg(default_value = "2026")]
        year: u16,
    },

    /// Show bulb hardware specs — lumens, wattage, CRI (bulbs only)
    Specs { host: String },

    /// Show saved light presets (bulbs only)
    Presets { host: String },

    /// Show scheduled rules (plugs only)
    Schedules { host: String },

    /// Control the plug's LED indicator (plugs only)
    Led {
        host: String,
        #[arg(value_enum)]
        state: LedAction,
    },

    /// Show device clock
    Clock { host: String },

    /// Rename a device
    Rename { host: String, name: String },

    /// Reboot a device
    Restart { host: String },

    /// List all outlets on a power strip with their state (strips only)
    Outlets { host: String },

    /// Show info about a Tapo device (uses TAPO_USER / TAPO_PASS env vars)
    Tapo { host: String },

    /// Turn a Tapo device on, off, or toggle (uses TAPO_USER / TAPO_PASS env vars)
    TapoPower {
        host: String,
        #[arg(value_enum)]
        state: PowerAction,
    },
}

#[derive(ValueEnum, Clone)]
enum PowerAction {
    On,
    Off,
    Toggle,
}

#[derive(ValueEnum, Clone)]
enum LedAction {
    On,
    Off,
}

/// Detect device type from sysinfo.
///
/// Newer devices use `mic_type`; older devices (HS110, HS105, etc.) use `type`.
/// Detection order for plug-type devices matters:
///   1. "Dimmer" in dev_name → Dimmer (HS220)
///   2. `children` array present → Strip (HS300, KP303, KP400)
///   3. Otherwise → Plug
/// For bulb-type devices:
///   1. `length` field present → LightStrip (KL430)
///   2. Otherwise → Bulb
enum DeviceKind {
    Bulb,
    LightStrip,
    Dimmer,
    Strip,
    Plug,
    Unknown(String),
}

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
        if has_length { DeviceKind::LightStrip } else { DeviceKind::Bulb }
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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Scan { timeout } => {
            println!("{}", format!("Scanning network for {timeout}s...").dimmed());
            let found = transport::broadcast(timeout).await?;
            if found.is_empty() {
                println!("No devices found.");
            } else {
                println!("Found {} device(s)\n", found.len());
                for (ip, json) in &found {
                    match detect_kind(json) {
                        DeviceKind::Bulb => {
                            if let Some(b) = bulb::parse(json) {
                                display::print_bulb_summary(*ip, &b);
                            }
                        }
                        DeviceKind::LightStrip => {
                            if let Some(b) = bulb::parse(json) {
                                display::print_lightstrip_summary(*ip, &b);
                            }
                        }
                        DeviceKind::Dimmer => {
                            if let Some(d) = dimmer::parse(json) {
                                display::print_dimmer_summary(*ip, &d);
                            }
                        }
                        DeviceKind::Strip => {
                            if let Some(s) = strip::parse(json) {
                                display::print_strip_summary(*ip, &s);
                            }
                        }
                        DeviceKind::Plug => {
                            if let Some(p) = plug::parse(json) {
                                display::print_plug_summary(*ip, &p);
                            }
                        }
                        DeviceKind::Unknown(t) => {
                            println!("{ip} - unknown device type: {t}");
                        }
                    }
                }
            }
        }

        Command::Info { host } => {
            let json = ops::sysinfo(&host).await?;
            match detect_kind(&json) {
                DeviceKind::Bulb => match bulb::parse(&json) {
                    Some(b) => display::print_bulb_detail(&host, &b),
                    None => bail!("Could not parse bulb sysinfo from {host}"),
                },
                DeviceKind::LightStrip => match bulb::parse(&json) {
                    Some(b) => display::print_lightstrip_detail(&host, &b),
                    None => bail!("Could not parse light strip sysinfo from {host}"),
                },
                DeviceKind::Dimmer => match dimmer::parse(&json) {
                    Some(d) => display::print_dimmer_detail(&host, &d),
                    None => bail!("Could not parse dimmer sysinfo from {host}"),
                },
                DeviceKind::Strip => match strip::parse(&json) {
                    Some(s) => display::print_strip_detail(&host, &s),
                    None => bail!("Could not parse strip sysinfo from {host}"),
                },
                DeviceKind::Plug => match plug::parse(&json) {
                    Some(p) => display::print_plug_detail(&host, &p),
                    None => bail!("Could not parse plug sysinfo from {host}"),
                },
                DeviceKind::Unknown(t) => bail!("Unknown device type at {host}: {t}"),
            }
        }

        Command::Power { host, state } => {
            let json = ops::sysinfo(&host).await?;
            let kind = detect_kind(&json);
            let use_bulb_ns = matches!(kind, DeviceKind::Bulb | DeviceKind::LightStrip);

            let result = match state {
                PowerAction::On => {
                    if use_bulb_ns { ops::bulb_on(&host).await? } else { ops::plug_on(&host).await? }
                    println!("{} {}", host, "on".green().bold());
                }
                PowerAction::Off => {
                    if use_bulb_ns { ops::bulb_off(&host).await? } else { ops::plug_off(&host).await? }
                    println!("{} {}", host, "off".dimmed());
                }
                PowerAction::Toggle => {
                    let now_on = if use_bulb_ns {
                        ops::bulb_toggle(&host).await?
                    } else {
                        ops::plug_toggle(&host).await?
                    };
                    let label = if now_on { "on".green().bold() } else { "off".dimmed() };
                    println!("{} toggled -> {}", host, label);
                }
            };
            result
        }

        Command::Dim { host, level } => {
            ops::set_brightness(&host, level).await?;
            println!("Brightness -> {level}%");
        }

        Command::Warmth { host, kelvin } => {
            ops::set_warmth(&host, kelvin).await?;
            println!("Color temperature -> {kelvin}K");
        }

        Command::Color { host, hue, saturation, value } => {
            ops::set_color(&host, hue, saturation, value).await?;
            println!("Color -> hue:{hue} sat:{saturation} val:{value}");
        }

        Command::Energy { host } => {
            let json = ops::sysinfo(&host).await?;
            // Check plug capability before calling — HS105 (TIM only) has no energy chip
            if let Some(p) = plug::parse(&json) {
                if !p.has_energy_monitoring() {
                    bail!("{} ({}) does not have energy monitoring (feature: {:?})",
                        p.alias, p.model, p.feature);
                }
            }
            let resp = match detect_kind(&json) {
                DeviceKind::Bulb => ops::bulb_energy(&host).await?,
                _ => ops::plug_energy(&host).await?,
            };
            display::print_energy_realtime(&resp);
        }

        Command::EnergyDaily { host, month } => {
            let parts: Vec<&str> = month.split('-').collect();
            if parts.len() != 2 {
                bail!("Month must be in YYYY-MM format");
            }
            let year: u16 = parts[0].parse()?;
            let mo: u8 = parts[1].parse()?;

            let json = ops::sysinfo(&host).await?;
            if let Some(p) = plug::parse(&json) {
                if !p.has_energy_monitoring() {
                    bail!("{} ({}) does not have energy monitoring", p.alias, p.model);
                }
            }
            let resp = match detect_kind(&json) {
                DeviceKind::Bulb => ops::bulb_energy_daily(&host, year, mo).await?,
                _ => ops::plug_energy_daily(&host, year, mo).await?,
            };
            display::print_energy_daily(&resp, &month);
        }

        Command::EnergyMonthly { host, year } => {
            let json = ops::sysinfo(&host).await?;
            if let Some(p) = plug::parse(&json) {
                if !p.has_energy_monitoring() {
                    bail!("{} ({}) does not have energy monitoring", p.alias, p.model);
                }
            }
            let resp = match detect_kind(&json) {
                DeviceKind::Bulb => ops::bulb_energy_monthly(&host, year).await?,
                _ => ops::plug_energy_monthly(&host, year).await?,
            };
            display::print_energy_monthly(&resp, year);
        }

        Command::Specs { host } => {
            let resp = ops::bulb_specs(&host).await?;
            display::print_bulb_specs(&resp);
        }

        Command::Presets { host } => {
            let resp = ops::bulb_presets(&host).await?;
            display::print_bulb_presets(&resp);
        }

        Command::Schedules { host } => {
            let resp = ops::plug_schedules(&host).await?;
            display::print_schedules(&resp);
        }

        Command::Led { host, state } => {
            let on = matches!(state, LedAction::On);
            ops::plug_led(&host, on).await?;
            println!("LED indicator {}", if on { "on".green() } else { "off".dimmed() });
        }

        Command::Clock { host } => {
            let resp = ops::plug_time(&host).await?;
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
            ops::rename(&host, &name).await?;
            println!("Renamed to \"{}\"", name.bold());
        }

        Command::Restart { host } => {
            ops::restart(&host).await?;
            println!("{} rebooting...", host);
        }

        Command::Outlets { host } => {
            let json = ops::sysinfo(&host).await?;
            match strip::parse(&json) {
                Some(s) => display::print_strip_outlets(&s),
                None => bail!("{host} does not appear to be a power strip"),
            }
        }
    }

    Ok(())
}
