use crate::commands::GroupAction;
use clap::{Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

#[derive(Parser)]
#[command(
    name = "denki",
    about = "Control TP-Link Kasa and Tapo devices from the terminal",
    version,
    after_help = "Start here:\n  denki scan\n  denki aliases\n  denki info \"desk plug\"\n  denki doctor \"desk plug\"\n\nTapo P125: add an alias with --tapo, then run denki login EMAIL.\nUse --json for one result; energy watch streams JSONL with --json.\nUse energy watch --format csv for spreadsheets; do not combine it with --json.\nOnly commands supported by the selected device are available.\nKasa scan names need explicit enrollment with denki alias NAME IP.\nSet DENKI_REQUIRE_TPAP=1 to reject KLAP fallback before authentication.\nHuman output escapes terminal controls; unsafe aliases have no command hints.\nGuide: https://kahwee.github.io/denki/"
)]
pub struct Cli {
    /// Emit versioned JSON (energy watch streams JSONL); progress goes to stderr
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Observe Kasa devices without enrolling names; probe saved Tapo aliases
    ///
    /// Kasa discovery cannot authenticate names or addresses. Verify each device's IP
    /// independently, then update its saved name with `denki alias NAME IP`.
    Scan {
        #[arg(short, long, default_value = "5")]
        timeout: u64,
        /// Probe a Tapo IP using its saved protocol, or auto TPAP/KLAP for a new IP
        #[arg(long, value_name = "IP")]
        tapo_target: Vec<std::net::IpAddr>,
    },

    /// Show detailed info about a device
    Info {
        /// Explicitly saved device alias or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
    },

    /// Diagnose connectivity, Wi-Fi signal, and capabilities with suggested next steps
    Doctor {
        /// Device name, saved alias, or IP address
        #[arg(
            value_name = "DEVICE",
            required_unless_present = "all",
            conflicts_with = "all"
        )]
        host: Option<String>,
        /// Check every saved address once without changing power or retrying failed checks
        #[arg(long)]
        all: bool,
        /// Show only failed addresses; summary and exit status still cover the whole fleet
        #[arg(long, requires = "all", conflicts_with = "host")]
        failed_only: bool,
        /// Maximum simultaneous fleet checks
        #[arg(long, default_value_t = 2, value_parser = clap::value_parser!(u8).range(1..=8), requires = "all", conflicts_with = "host")]
        concurrency: u8,
        /// Deadline in seconds for each device
        #[arg(long, default_value_t = 20, value_parser = clap::value_parser!(u16).range(1..=120))]
        timeout: u16,
    },

    /// Record or clear your physical identification of an exact saved alias (no device I/O)
    Identify {
        name: String,
        /// Confirm you have physically identified this device
        #[arg(long, required_unless_present = "clear", conflicts_with = "clear")]
        confirm: bool,
        /// Clear an earlier physical identification
        #[arg(long)]
        clear: bool,
    },

    /// Turn a device on
    On {
        /// Explicitly saved device alias or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        /// Outlet number, 1-based (strips only)
        #[arg(value_parser = clap::value_parser!(u8).range(1..))]
        outlet: Option<u8>,
    },

    /// Turn a device off
    Off {
        /// Explicitly saved device alias or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        /// Outlet number, 1-based (strips only)
        #[arg(value_parser = clap::value_parser!(u8).range(1..))]
        outlet: Option<u8>,
    },

    /// Toggle a device on/off
    Toggle {
        /// Explicitly saved device alias or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        /// Outlet number, 1-based (strips only)
        #[arg(value_parser = clap::value_parser!(u8).range(1..))]
        outlet: Option<u8>,
    },

    /// Apply on/off/toggle to matching aliases (Kasa requires explicit alias enrollment)
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
        /// Explicitly saved device alias or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        #[arg(value_parser = clap::value_parser!(u8).range(0..=100))]
        level: u8,
    },

    /// Set color temperature in Kelvin 2500-9000 (bulbs only)
    ColorTemp {
        /// Explicitly saved device alias or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        #[arg(value_parser = clap::value_parser!(u16).range(2500..=9000))]
        kelvin: u16,
    },

    /// Set HSV color (bulbs only)
    Color {
        /// Explicitly saved device alias or an IP address
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

    /// Cycle a color light through the rainbow until interrupted with Ctrl-C
    Rainbow {
        /// Explicitly saved device alias or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        /// Delay between hue steps, in milliseconds
        #[arg(long, default_value_t = 1_000, value_parser = clap::value_parser!(u64).range(20..=60_000))]
        step_delay_ms: u64,
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
        /// Explicitly saved device alias or an IP address
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
        /// Explicitly saved device alias or an IP address
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
        /// Explicitly saved device alias or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
    },

    /// Show saved light presets (bulbs only)
    Presets {
        /// Explicitly saved device alias or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
    },

    /// List built-in light strip effects and the current active effect (light strips only)
    Effects {
        /// Explicitly saved device alias or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
    },

    /// Activate a built-in light strip effect by name (light strips only)
    Effect {
        /// Explicitly saved device alias or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        /// Effect name, e.g. Aurora, Rainbow, Off
        name: String,
    },

    /// Show scheduled rules (plugs, dimmers, and power strips)
    Schedules {
        /// Explicitly saved device alias or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
    },

    /// Control the LED indicator (plugs, dimmers, and power strips)
    Led {
        /// Explicitly saved device alias or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        #[arg(value_enum)]
        state: LedAction,
    },

    /// Show device clock (plugs, dimmers, and power strips)
    Clock {
        /// Explicitly saved device alias or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
    },

    /// Rename a Kasa device
    Rename {
        /// Explicitly saved device alias or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        name: String,
    },

    /// Reboot a Kasa device
    Restart {
        /// Explicitly saved device alias or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
    },

    /// List all outlets on a power strip, showing 1-based outlet numbers, names, and state (strips only)
    Outlets {
        /// Explicitly saved device alias or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
    },

    /// Rename one outlet on a power strip (1-based outlet number)
    OutletRename {
        /// Explicitly saved device alias or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        /// Outlet number, 1-based
        #[arg(value_parser = clap::value_parser!(u8).range(1..))]
        outlet: u8,
        /// New name for the outlet
        name: String,
    },

    /// Save a friendly name for a device IP (e.g. `denki alias "floor lamp" 192.0.2.42`)
    Alias {
        /// Friendly name to save
        name: String,
        /// IP address of the device
        ip: String,
        /// Mark as a Tapo device (uses KLAP protocol on port 80)
        #[arg(long)]
        klap: bool,
        /// Auto-negotiate TPAP/KLAP with the upstream plug client (P125 power support)
        #[arg(long, visible_alias = "tpap", conflicts_with = "klap")]
        tapo: bool,
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
        /// Read one password line from redirected stdin instead of the hidden prompt
        #[arg(long)]
        password_stdin: bool,
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
    fn tapo_alias_options_are_exclusive_and_support_tpap_spelling() {
        for flag in ["--tapo", "--tpap"] {
            let cli =
                Cli::try_parse_from(["denki", "alias", "Test Plug", "192.0.2.1", flag]).unwrap();
            assert!(matches!(
                cli.command,
                Command::Alias {
                    tapo: true,
                    klap: false,
                    ..
                }
            ));
            assert!(
                Cli::try_parse_from(["denki", "alias", "Test Plug", "192.0.2.1", flag, "--klap"])
                    .is_err()
            );
        }
        let cli =
            Cli::try_parse_from(["denki", "alias", "Test Plug", "192.0.2.1", "--klap"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Alias {
                tapo: false,
                klap: true,
                ..
            }
        ));
    }

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
    fn diagnostics_and_identification_reject_ambiguous_or_unbounded_requests() {
        for args in [
            vec!["doctor"],
            vec!["doctor", "desk", "--all"],
            vec!["doctor", "desk", "--failed-only"],
            vec!["doctor", "--failed-only"],
            vec!["doctor", "--all", "--concurrency", "0"],
            vec!["doctor", "--all", "--concurrency", "9"],
            vec!["doctor", "desk", "--concurrency", "2"],
            vec!["doctor", "desk", "--timeout", "0"],
            vec!["doctor", "desk", "--timeout", "121"],
            vec!["identify", "desk"],
            vec!["identify", "desk", "--confirm", "--clear"],
        ] {
            assert!(
                Cli::try_parse_from(std::iter::once("denki").chain(args.iter().copied())).is_err(),
                "{args:?}"
            );
        }
        for args in [
            vec!["doctor", "desk", "--json"],
            vec!["doctor", "--all", "--failed-only", "--json"],
            vec!["doctor", "--all", "--concurrency", "8", "--timeout", "120"],
            vec!["identify", "desk", "--confirm"],
            vec!["identify", "desk", "--clear"],
        ] {
            assert!(
                Cli::try_parse_from(std::iter::once("denki").chain(args.iter().copied())).is_ok(),
                "{args:?}"
            );
        }
    }
}

impl Command {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Scan { .. } => "scan",
            Self::Info { .. } => "info",
            Self::Doctor { .. } => "doctor",
            Self::Identify { .. } => "identify",
            Self::On { .. } => "on",
            Self::Off { .. } => "off",
            Self::Toggle { .. } => "toggle",
            Self::Group { .. } => "group",
            Self::Dim { .. } => "dim",
            Self::ColorTemp { .. } => "color-temp",
            Self::Color { .. } => "color",
            Self::Rainbow { .. } => "rainbow",
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
