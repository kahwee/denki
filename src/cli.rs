use clap::{Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

#[derive(Parser)]
#[command(
    name = "denki",
    about = "Control TP-Link Kasa and Tapo devices from the terminal",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
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

    /// Show real-time energy usage (bulbs, light strips, and ENE-capable plugs/strips)
    Energy {
        /// Device name from scan output, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        /// Outlet number, 1-based (strips only)
        #[arg(value_parser = clap::value_parser!(u8).range(1..))]
        outlet: Option<u8>,
    },

    /// Show daily energy usage for a month (YYYY-MM) on bulbs, light strips, and ENE-capable plugs/strips
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

    /// Show monthly energy usage for a year on bulbs, light strips, and ENE-capable plugs/strips
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

    /// Rename a Kasa device
    Rename {
        /// Device name from scan output, or an IP address
        #[arg(value_name = "DEVICE")]
        host: String,
        name: String,
    },

    /// Reboot a Kasa device
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

#[derive(ValueEnum, Clone)]
pub enum LedAction {
    On,
    Off,
}
