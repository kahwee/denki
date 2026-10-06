use crate::commands::GroupAction;
use clap::{Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

#[derive(Parser)]
#[command(
    name = "denki",
    about = "Control TP-Link Kasa and Tapo devices from the terminal",
    version
)]
pub struct Cli {
    /// Emit one versioned JSON result; progress goes to stderr
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Scan Kasa devices and saved Tapo aliases; save new names without replacing existing aliases
    Scan {
        #[arg(short, long, default_value = "5")]
        timeout: u64,
        /// Probe an additional Tapo address and reconcile its saved identity
        #[arg(long, value_name = "IP")]
        tapo_target: Vec<std::net::IpAddr>,
    },

    /// Show detailed info about a device
    Info {
        /// Device name from scan output, a saved alias, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
    },

    /// Check reachability, protocol, parsing, and advertised capabilities
    Doctor {
        /// Device name, saved alias, or IP address
        #[arg(value_name = "DEVICE")]
        host: String,
    },

    /// Turn a device on
    On {
        /// Device name from scan output, a saved alias, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        /// Outlet number, 1-based (strips only)
        #[arg(value_parser = clap::value_parser!(u8).range(1..))]
        outlet: Option<u8>,
    },

    /// Turn a device off
    Off {
        /// Device name from scan output, a saved alias, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        /// Outlet number, 1-based (strips only)
        #[arg(value_parser = clap::value_parser!(u8).range(1..))]
        outlet: Option<u8>,
    },

    /// Toggle a device on/off
    Toggle {
        /// Device name from scan output, a saved alias, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        /// Outlet number, 1-based (strips only)
        #[arg(value_parser = clap::value_parser!(u8).range(1..))]
        outlet: Option<u8>,
    },

    /// Apply on/off/toggle to all matching aliases
    Group {
        #[arg(value_enum)]
        action: GroupAction,
        /// Alias pattern used for group matching (normalized, case- and punctuation-insensitive)
        #[arg(value_name = "PATTERN")]
        pattern: String,
        /// Show matched aliases without changing device state
        #[arg(long)]
        dry_run: bool,
        /// Maximum number of devices controlled at once
        #[arg(long, default_value_t = 4, value_parser = clap::value_parser!(u8).range(1..=32))]
        concurrency: u8,
    },

    /// Set brightness 0-100 (bulbs and HS220 dimmers)
    Dim {
        /// Device name from scan output, a saved alias, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        #[arg(value_parser = clap::value_parser!(u8).range(0..=100))]
        level: u8,
    },

    /// Set color temperature in Kelvin 2500-9000 (bulbs only)
    ColorTemp {
        /// Device name from scan output, a saved alias, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        #[arg(value_parser = clap::value_parser!(u16).range(2500..=9000))]
        kelvin: u16,
    },

    /// Set HSV color (bulbs only)
    Color {
        /// Device name from scan output, a saved alias, or an IP address
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

    /// Show real-time energy usage (bulbs, light strips, and ENE-capable plugs/strips)
    #[command(
        subcommand_precedence_over_arg = true,
        args_conflicts_with_subcommands = true
    )]
    Energy {
        #[command(subcommand)]
        command: Option<EnergyCommand>,
        /// Device name, saved alias, or IP; use `energy watch` for streaming
        #[arg(value_name = "DEVICE")]
        host: Option<String>,
        /// Outlet number, 1-based (strips only)
        #[arg(value_parser = clap::value_parser!(u8).range(1..))]
        outlet: Option<u8>,
    },

    /// Show daily energy usage for a month (YYYY-MM) on bulbs, light strips, and ENE-capable plugs/strips
    EnergyDaily {
        /// Device name from scan output, a saved alias, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        /// Month in YYYY-MM format (defaults to current month)
        #[arg(value_name = "YYYY-MM")]
        month: Option<String>,
        /// Outlet number, 1-based (strips only)
        #[arg(long, short = 'o', value_parser = clap::value_parser!(u8).range(1..))]
        outlet: Option<u8>,
    },

    /// Show monthly energy usage for a year on bulbs, light strips, and ENE-capable plugs/strips
    EnergyMonthly {
        /// Device name from scan output, a saved alias, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        #[arg(value_name = "YYYY")]
        year: Option<u16>,
        /// Outlet number, 1-based (strips only)
        #[arg(long, short = 'o', value_parser = clap::value_parser!(u8).range(1..))]
        outlet: Option<u8>,
    },

    /// Show bulb hardware specs — lumens, wattage, CRI (bulbs only)
    Specs {
        /// Device name from scan output, a saved alias, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
    },

    /// Show saved light presets (bulbs only)
    Presets {
        /// Device name from scan output, a saved alias, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
    },

    /// List built-in light strip effects and the current active effect (light strips only)
    Effects {
        /// Device name from scan output, a saved alias, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
    },

    /// Activate a built-in light strip effect by name (light strips only)
    Effect {
        /// Device name from scan output, a saved alias, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        /// Effect name, e.g. Aurora, Rainbow, Off
        name: String,
    },

    /// Show scheduled rules (plugs, dimmers, and power strips)
    Schedules {
        /// Device name from scan output, a saved alias, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
    },

    /// Control the LED indicator (plugs, dimmers, and power strips)
    Led {
        /// Device name from scan output, a saved alias, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        #[arg(value_enum)]
        state: LedAction,
    },

    /// Show device clock (plugs, dimmers, and power strips)
    Clock {
        /// Device name from scan output, a saved alias, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
    },

    /// Rename a Kasa device
    Rename {
        /// Device name from scan output, a saved alias, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        name: String,
    },

    /// Reboot a Kasa device
    Restart {
        /// Device name from scan output, a saved alias, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
    },

    /// List all outlets on a power strip, showing 1-based outlet numbers, names, and state (strips only)
    Outlets {
        /// Device name from scan output, a saved alias, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
    },

    /// Rename one outlet on a power strip (1-based outlet number)
    OutletRename {
        /// Device name from scan output, a saved alias, or an IP address
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

    /// Print shell completions to stdout (pipe to your shell's completions dir)
    #[command(hide = true)]
    Completions {
        #[arg(value_enum)]
        shell: Shell,
    },

    /// Save Tapo account credentials to avoid setting env vars each session
    Login {
        /// Tapo account email address
        email: String,
        /// Tapo account password (omit to be prompted; never pass on command line in scripts)
        password: Option<String>,
    },
}

#[derive(ValueEnum, Clone, Debug)]
pub enum LedAction {
    On,
    Off,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_safety_options_parse() {
        let cli = Cli::try_parse_from([
            "denki",
            "group",
            "off",
            "office",
            "--dry-run",
            "--concurrency",
            "8",
        ])
        .unwrap();
        match cli.command {
            Command::Group {
                dry_run,
                concurrency,
                ..
            } => {
                assert!(dry_run);
                assert_eq!(concurrency, 8);
            }
            _ => panic!("expected group command"),
        }
    }

    #[test]
    fn doctor_json_option_parses() {
        let cli = Cli::try_parse_from(["denki", "doctor", "office", "--json"]).unwrap();
        assert!(cli.json);
    }

    #[test]
    fn info_json_option_parses() {
        let cli = Cli::try_parse_from(["denki", "info", "office", "--json"]).unwrap();
        assert!(cli.json);
    }
}

impl Command {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Scan { .. } => "scan",
            Self::Info { .. } => "info",
            Self::Doctor { .. } => "doctor",
            Self::On { .. } => "on",
            Self::Off { .. } => "off",
            Self::Toggle { .. } => "toggle",
            Self::Group { .. } => "group",
            Self::Dim { .. } => "dim",
            Self::ColorTemp { .. } => "color-temp",
            Self::Color { .. } => "color",
            Self::Energy { .. } => "energy",
            Self::EnergyDaily { .. } => "energy-daily",
            Self::EnergyMonthly { .. } => "energy-monthly",
            Self::Specs { .. } => "specs",
            Self::Presets { .. } => "presets",
            Self::Effects { .. } => "effects",
            Self::Effect { .. } => "effect",
            Self::Schedules { .. } => "schedules",
            Self::Led { .. } => "led",
            Self::Clock { .. } => "clock",
            Self::Rename { .. } => "rename",
            Self::Restart { .. } => "restart",
            Self::Outlets { .. } => "outlets",
            Self::OutletRename { .. } => "outlet-rename",
            Self::Alias { .. } => "alias",
            Self::Unalias { .. } => "unalias",
            Self::Aliases => "aliases",
            Self::Completions { .. } => "completions",
            Self::Login { .. } => "login",
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum EnergyCommand {
    /// Poll read-only measurements; export with --format jsonl or csv
    Watch {
        #[arg(value_name = "DEVICE")]
        host: String,
        #[arg(long, short = 'o', value_parser = clap::value_parser!(u8).range(1..))]
        outlet: Option<u8>,
        /// Delay in seconds after each completed sample (no overlapping requests)
        #[arg(long, default_value_t = 5, value_parser = clap::value_parser!(u64).range(1..=86400))]
        interval: u64,
        /// Stop after this many samples; omit to run until Ctrl-C
        #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
        count: Option<u64>,
        #[arg(long, value_enum, default_value_t = WatchFormat::Table)]
        format: WatchFormat,
    },
}
#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq)]
pub enum WatchFormat {
    Table,
    Jsonl,
    Csv,
}
