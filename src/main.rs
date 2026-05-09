use denki::{bulb, creds, dimmer, display, hosts, klap, ops, plug, strip, tapo, transport};

use anyhow::{bail, Result};
use clap::{Parser, Subcommand, ValueEnum};
use colored::Colorize;
use std::net::IpAddr;

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
    },

    /// Turn a device off
    Off {
        /// Device name from scan output, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
    },

    /// Toggle a device on/off
    Toggle {
        /// Device name from scan output, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
    },

    /// Set brightness 0-100 (bulbs only)
    Dim {
        /// Device name from scan output, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        #[arg(value_parser = clap::value_parser!(u8).range(0..=100))]
        level: u8,
    },

    /// Set color temperature in Kelvin 2500-9000 (bulbs only)
    Warmth {
        /// Device name from scan output, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        #[arg(value_parser = clap::value_parser!(u16).range(2500..=9000))]
        kelvin: u16,
    },

    /// Set color in HSV — hue 0-360, saturation 0-100, value 0-100 (bulbs only)
    Color {
        /// Device name from scan output, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        #[arg(value_parser = clap::value_parser!(u16).range(0..=360))]
        hue: u16,
        #[arg(value_parser = clap::value_parser!(u8).range(0..=100))]
        saturation: u8,
        #[arg(value_parser = clap::value_parser!(u8).range(0..=100))]
        value: u8,
    },

    /// Show real-time energy usage
    Energy {
        /// Device name from scan output, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
    },

    /// Show daily energy usage for a month (YYYY-MM)
    EnergyDaily {
        /// Device name from scan output, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        /// Month in YYYY-MM format (defaults to current month)
        month: Option<String>,
    },

    /// Show monthly energy usage for a year (plugs only)
    EnergyMonthly {
        /// Device name from scan output, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        year: Option<u16>,
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

    /// Show scheduled rules (plugs only)
    Schedules {
        /// Device name from scan output, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
    },

    /// Control the plug's LED indicator (plugs only)
    Led {
        /// Device name from scan output, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        #[arg(value_enum)]
        state: LedAction,
    },

    /// Show device clock
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

    /// List all outlets on a power strip with their state (strips only)
    Outlets {
        /// Device name from scan output, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
    },

    /// Turn one outlet on a power strip on, off, or toggle it (strips only)
    Outlet {
        /// Device name from scan output, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        /// Outlet number, 1-based
        #[arg(value_parser = clap::value_parser!(u8).range(1..))]
        outlet: u8,
        #[arg(value_enum)]
        state: PowerAction,
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
        /// Tapo account password
        password: String,
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
///
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

impl std::fmt::Display for DeviceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceKind::Bulb => write!(f, "bulb"),
            DeviceKind::LightStrip => write!(f, "light strip"),
            DeviceKind::Dimmer => write!(f, "dimmer"),
            DeviceKind::Strip => write!(f, "power strip"),
            DeviceKind::Plug => write!(f, "plug"),
            DeviceKind::Unknown(t) => write!(f, "unknown ({t})"),
        }
    }
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

// ── Command compatibility guards ──────────────────────────────────────────────
// Pure synchronous functions: take a DeviceKind, return Ok or a clear error.
// Handlers call these before issuing any network request so the user always
// gets a command-level message ("dim is not supported on plug") rather than
// a raw protocol error from the device.

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

fn can_set_warmth(kind: &DeviceKind) -> Result<()> {
    match kind {
        DeviceKind::Bulb => Ok(()),
        other => anyhow::bail!(
            "`warmth` is only supported on KL135-style color bulbs (e.g. KL135), not {other}"
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
        other => anyhow::bail!(
            "`specs` is only supported on KL135-style bulbs, not {other}"
        ),
    }
}

fn can_get_presets(kind: &DeviceKind) -> Result<()> {
    match kind {
        DeviceKind::Bulb => Ok(()),
        other => anyhow::bail!(
            "`presets` is only supported on KL135-style bulbs, not {other}"
        ),
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
        DeviceKind::Plug | DeviceKind::Dimmer => Ok(()),
        other => anyhow::bail!(
            "`led` is only supported on plugs and dimmers (e.g. KP115, HS220), not {other}"
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
/// - Bulb / LightStrip: always supported (smartlife.iot.common.emeter)
/// - Plug: only if it has the ENE feature flag (KP115/HS110 yes, HS105 no)
/// - Dimmer / Strip / Unknown: not supported — bail with a clear message
fn require_energy(json: &serde_json::Value, kind: &DeviceKind) -> Result<()> {
    match kind {
        DeviceKind::Bulb | DeviceKind::LightStrip => Ok(()),
        DeviceKind::Plug => {
            if let Some(p) = plug::parse(json) {
                if !p.has_energy_monitoring() {
                    anyhow::bail!(
                        "{} ({}) does not have energy monitoring (feature: {:?})",
                        p.alias,
                        p.model,
                        p.feature
                    );
                }
            }
            Ok(())
        }
        other => anyhow::bail!("{other} does not support energy monitoring"),
    }
}

fn device_alias(json: &serde_json::Value) -> Option<&str> {
    json.pointer("/system/get_sysinfo/alias")
        .and_then(|v| v.as_str())
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
struct Resolved {
    ip: String,
    protocol: hosts::Protocol,
}

/// Resolve a name or IP to (ip, protocol).
/// Resolution order:
///   1. Already an IP → Kasa (default)
///   2. Saved alias in hosts file → uses stored protocol
///   3. Live UDP scan → Kasa
async fn resolve(input: &str) -> Result<Resolved> {
    if input.parse::<IpAddr>().is_ok() || input.contains('.') {
        return Ok(Resolved { ip: input.to_string(), protocol: hosts::Protocol::Kasa });
    }
    if let Some(entry) = hosts::lookup(input) {
        println!("{}", format!("Using alias \"{input}\" [{}]", entry.ip).dimmed());
        return Ok(Resolved { ip: entry.ip, protocol: entry.protocol });
    }
    println!("{}", format!("Resolving \"{input}\"...").dimmed());
    let found = transport::broadcast(3).await?;
    let matches: Vec<_> = found
        .iter()
        .filter_map(|(ip, json)| {
            let alias = device_alias(json)?;
            let a = hosts::normalize(alias);
            let q = hosts::normalize(input);
            (!q.is_empty() && (a == q || a.contains(&q))).then_some((*ip, alias.to_string()))
        })
        .collect();

    match matches.as_slice() {
        [(ip, alias)] => {
            println!("{}", format!("Using {alias} [{ip}]").dimmed());
            Ok(Resolved { ip: ip.to_string(), protocol: hosts::Protocol::Kasa })
        }
        [] => bail!("No device named \"{input}\" found. Run `denki scan` to see available names."),
        many => {
            let names = many
                .iter()
                .map(|(ip, alias)| format!("{alias} [{ip}]"))
                .collect::<Vec<_>>()
                .join(", ");
            bail!("\"{input}\" matched multiple devices: {names}")
        }
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
                        DeviceKind::Unknown(t) => bail!("Unknown device type at {}: {t}", r.ip),
                    }
                }
            }
        }

        Command::On { host } => {
            let r = resolve(&host).await?;
            match r.protocol {
                hosts::Protocol::Klap => {
                    let mut session = open_tapo(&r.ip).await?;
                    ops::tapo_on(&mut session).await?;
                }
                hosts::Protocol::Kasa => {
                    let json = ops::sysinfo(&r.ip).await?;
                    let kind = detect_kind(&json);
                    if matches!(kind, DeviceKind::LightStrip) {
                        bail!("light strip power control is not yet implemented (KL430 uses smartlife.iot.lightStrip)");
                    }
                    if matches!(kind, DeviceKind::Bulb) { ops::bulb_on(&r.ip).await? } else { ops::plug_on(&r.ip).await? }
                }
            }
            println!("{} {}", r.ip, "on".green().bold());
        }

        Command::Off { host } => {
            let r = resolve(&host).await?;
            match r.protocol {
                hosts::Protocol::Klap => {
                    let mut session = open_tapo(&r.ip).await?;
                    ops::tapo_off(&mut session).await?;
                }
                hosts::Protocol::Kasa => {
                    let json = ops::sysinfo(&r.ip).await?;
                    let kind = detect_kind(&json);
                    if matches!(kind, DeviceKind::LightStrip) {
                        bail!("light strip power control is not yet implemented (KL430 uses smartlife.iot.lightStrip)");
                    }
                    if matches!(kind, DeviceKind::Bulb) { ops::bulb_off(&r.ip).await? } else { ops::plug_off(&r.ip).await? }
                }
            }
            println!("{} {}", r.ip, "off".dimmed());
        }

        Command::Toggle { host } => {
            let r = resolve(&host).await?;
            let now_on = match r.protocol {
                hosts::Protocol::Klap => {
                    let mut session = open_tapo(&r.ip).await?;
                    ops::tapo_toggle(&mut session).await?
                }
                hosts::Protocol::Kasa => {
                    let json = ops::sysinfo(&r.ip).await?;
                    let kind = detect_kind(&json);
                    if matches!(kind, DeviceKind::LightStrip) {
                        bail!("light strip power control is not yet implemented (KL430 uses smartlife.iot.lightStrip)");
                    }
                    if matches!(kind, DeviceKind::Bulb) { ops::bulb_toggle(&r.ip).await? } else { ops::plug_toggle(&r.ip).await? }
                }
            };
            let label = if now_on { "on".green().bold() } else { "off".dimmed() };
            println!("{} -> {label}", r.ip);
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
                    ops::set_brightness(&r.ip, level).await?;
                }
                DeviceKind::Dimmer => {
                    // Turn on first if currently off and a non-zero level was requested
                    if level > 0 && dimmer::parse(&json).is_some_and(|d| !d.is_on()) {
                        ops::plug_on(&r.ip).await?;
                    }
                    ops::dimmer_set_brightness(&r.ip, level).await?;
                }
                _ => unreachable!(),
            }
            println!("Brightness -> {level}%");
        }

        Command::Warmth { host, kelvin } => {
            let r = resolve(&host).await?;
            let json = ops::sysinfo(&r.ip).await?;
            can_set_warmth(&detect_kind(&json))?;
            ops::set_warmth(&r.ip, kelvin).await?;
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
            ops::set_color(&r.ip, hue, saturation, value).await?;
            println!("Color -> hue:{hue} sat:{saturation} val:{value}");
        }

        Command::Energy { host } => {
            let r = resolve(&host).await?;
            let json = ops::sysinfo(&r.ip).await?;
            let kind = detect_kind(&json);
            require_energy(&json, &kind)?;
            let resp = match &kind {
                DeviceKind::Bulb | DeviceKind::LightStrip => ops::bulb_energy(&r.ip).await?,
                _ => ops::plug_energy(&r.ip).await?,
            };
            display::print_energy_realtime(&resp);
        }

        Command::EnergyDaily { host, month } => {
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

            let json = ops::sysinfo(&host).await?;
            let kind = detect_kind(&json);
            require_energy(&json, &kind)?;
            let resp = match &kind {
                DeviceKind::Bulb | DeviceKind::LightStrip => {
                    ops::bulb_energy_daily(&host, year, mo).await?
                }
                _ => ops::plug_energy_daily(&host, year, mo).await?,
            };
            display::print_energy_daily(&resp, &month_str);
        }

        Command::EnergyMonthly { host, year } => {
            let r = resolve(&host).await?;
            let year = year.unwrap_or_else(|| current_year_month().0);
            let json = ops::sysinfo(&r.ip).await?;
            let kind = detect_kind(&json);
            require_energy(&json, &kind)?;
            let resp = match &kind {
                DeviceKind::Bulb | DeviceKind::LightStrip => {
                    ops::bulb_energy_monthly(&r.ip, year).await?
                }
                _ => ops::plug_energy_monthly(&r.ip, year).await?,
            };
            display::print_energy_monthly(&resp, year);
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
            let resp = ops::plug_schedules(&r.ip).await?;
            display::print_schedules(&resp);
        }

        Command::Led { host, state } => {
            let r = resolve(&host).await?;
            let json = ops::sysinfo(&r.ip).await?;
            can_control_led(&detect_kind(&json))?;
            let on = matches!(state, LedAction::On);
            ops::plug_led(&r.ip, on).await?;
            println!("LED indicator {}", if on { "on".green() } else { "off".dimmed() });
        }

        Command::Clock { host } => {
            let r = resolve(&host).await?;
            let json = ops::sysinfo(&r.ip).await?;
            can_get_clock(&detect_kind(&json))?;
            let resp = ops::plug_time(&r.ip).await?;
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

        Command::Outlet { host, outlet, state } => {
            let r = resolve(&host).await?;
            let json = ops::sysinfo(&r.ip).await?;
            let s = strip::parse(&json)
                .ok_or_else(|| anyhow::anyhow!("{} does not appear to be a power strip", r.ip))?;
            let idx = (outlet - 1) as usize;
            let child = s.children.get(idx).ok_or_else(|| {
                anyhow::anyhow!(
                    "outlet {} does not exist (strip has {} outlets)",
                    outlet,
                    s.children.len()
                )
            })?;
            let now_on = match state {
                PowerAction::On => {
                    ops::strip_outlet_on(&r.ip, &child.id).await?;
                    true
                }
                PowerAction::Off => {
                    ops::strip_outlet_off(&r.ip, &child.id).await?;
                    false
                }
                PowerAction::Toggle => {
                    if child.is_on() {
                        ops::strip_outlet_off(&r.ip, &child.id).await?;
                        false
                    } else {
                        ops::strip_outlet_on(&r.ip, &child.id).await?;
                        true
                    }
                }
            };
            let label = if now_on { "on".green().bold() } else { "off".dimmed() };
            println!("Outlet {} ({}) -> {}", outlet, child.alias, label);
        }

        Command::Alias { name, ip, klap } => {
            let protocol = if klap { hosts::Protocol::Klap } else { hosts::Protocol::Kasa };
            hosts::set(&name, &ip, protocol)?;
            let tag = if klap { " (klap)".dimmed() } else { "".normal() };
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
                println!("{:<30} {:<18} {}", "Name".bold(), "IP".bold(), "Protocol".bold());
                println!("{}", "─".repeat(58).dimmed());
                for (name, entry) in &list {
                    println!("{:<30} {:<18} {}", name, entry.ip, entry.protocol);
                }
                println!("{}", format!("({} aliases in {})", list.len(), hosts::path_display()).dimmed());
            }
        }

        Command::Login { email, password } => {
            creds::save(&email, &password)?;
            println!("Tapo credentials saved to {}", creds::path_display());
            println!("(File is readable only by you. Use TAPO_USER/TAPO_PASS env vars to override.)");
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

    #[test]
    fn device_alias_reads_legacy_sysinfo_alias() {
        let json = json!({
            "system": {
                "get_sysinfo": {
                    "alias": "Coat Rack Lights"
                }
            }
        });

        assert_eq!(device_alias(&json), Some("Coat Rack Lights"));
    }

    // ── Command guard tests ───────────────────────────────────────────────────
    //
    // Each guard function is pure (no I/O), so we test it exhaustively here.
    // The handlers call these before any network request, so these tests cover
    // the routing decisions without needing a live device.

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
        assert!(err.to_string().contains("`dim`"), "error should name the command: {err}");
    }

    #[test]
    fn can_dim_lightstrip_error_explains_namespace() {
        let err = can_dim(&DeviceKind::LightStrip).unwrap_err();
        assert!(err.to_string().contains("not yet supported"), "{err}");
        assert!(err.to_string().contains("lightStrip"), "{err}");
    }

    #[test]
    fn can_set_warmth_accepts_bulb_only() {
        assert!(can_set_warmth(&DeviceKind::Bulb).is_ok());
        assert!(can_set_warmth(&DeviceKind::Dimmer).is_err());
        assert!(can_set_warmth(&DeviceKind::Plug).is_err());
        assert!(can_set_warmth(&DeviceKind::LightStrip).is_err());
        assert!(can_set_warmth(&DeviceKind::Strip).is_err());
    }

    #[test]
    fn can_set_warmth_error_mentions_kl135() {
        let err = can_set_warmth(&DeviceKind::Plug).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("`warmth`"), "{msg}");
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
        for kind in [DeviceKind::Dimmer, DeviceKind::Plug, DeviceKind::Strip, DeviceKind::LightStrip] {
            assert!(can_get_specs(&kind).is_err(), "specs should reject {kind}");
            assert!(can_get_presets(&kind).is_err(), "presets should reject {kind}");
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
        assert!(msg.contains("KP115") || msg.contains("HS220") || msg.contains("HS300"), "{msg}");
    }

    #[test]
    fn can_control_led_accepts_plug_and_dimmer() {
        assert!(can_control_led(&DeviceKind::Plug).is_ok());
        assert!(can_control_led(&DeviceKind::Dimmer).is_ok());
        assert!(can_control_led(&DeviceKind::Bulb).is_err());
        assert!(can_control_led(&DeviceKind::Strip).is_err());
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

    // ── CLI parsing tests ─────────────────────────────────────────────────────

    #[test]
    fn on_off_toggle_parse_as_top_level_commands() {
        let on = Cli::try_parse_from(["denki", "on", "desk lamp"]).unwrap();
        assert!(matches!(on.command, Command::On { host } if host == "desk lamp"));

        let off = Cli::try_parse_from(["denki", "off", "desk lamp"]).unwrap();
        assert!(matches!(off.command, Command::Off { host } if host == "desk lamp"));

        let tog = Cli::try_parse_from(["denki", "toggle", "desk lamp"]).unwrap();
        assert!(matches!(tog.command, Command::Toggle { host } if host == "desk lamp"));
    }

    #[test]
    fn power_subcommand_no_longer_exists() {
        // `power` was replaced by `on`/`off`/`toggle` — clap should reject it
        assert!(Cli::try_parse_from(["denki", "power", "desk lamp", "on"]).is_err());
    }

    #[test]
    fn dim_command_parses_host_and_level() {
        let cli = Cli::try_parse_from(["denki", "dim", "desk lamp", "75"]).unwrap();
        assert!(matches!(cli.command, Command::Dim { host, level } if host == "desk lamp" && level == 75));
    }

    #[test]
    fn dim_rejects_level_above_100() {
        assert!(Cli::try_parse_from(["denki", "dim", "desk lamp", "101"]).is_err());
    }

    #[test]
    fn outlet_command_parses_host_outlet_and_state() {
        let cli = Cli::try_parse_from(["denki", "outlet", "strip", "2", "on"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Outlet { host, outlet, state: PowerAction::On } if host == "strip" && outlet == 2
        ));
    }

    #[test]
    fn outlet_rejects_zero_index() {
        assert!(Cli::try_parse_from(["denki", "outlet", "strip", "0", "on"]).is_err());
    }
}
