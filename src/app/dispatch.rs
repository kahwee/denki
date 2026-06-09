use anyhow::Result;
use clap::Parser;

use crate::admin;
use crate::cli::{Cli, Command};
use crate::commands;
use crate::effects;

use super::info::handle_info;
use super::scan::handle_scan;

async fn dispatch_command(command: Command) -> Result<()> {
    match command {
        Command::Scan { timeout } => handle_scan(timeout).await,
        Command::Info { host } => handle_info(host).await,
        Command::On { host, outlet } => commands::handle_on(&host, outlet).await,
        Command::Off { host, outlet } => commands::handle_off(&host, outlet).await,
        Command::Toggle { host, outlet } => commands::handle_toggle(&host, outlet).await,
        Command::Dim { host, level } => commands::handle_dim(&host, level).await,
        Command::ColorTemp { host, kelvin } => commands::handle_color_temp(&host, kelvin).await,
        Command::Color {
            host,
            hue,
            saturation,
            value,
        } => commands::handle_color(&host, hue, saturation, value).await,
        Command::Energy { host, outlet } => commands::handle_energy(&host, outlet).await,
        Command::EnergyDaily {
            host,
            month,
            outlet,
        } => commands::handle_energy_daily(&host, month, outlet).await,
        Command::EnergyMonthly { host, year, outlet } => {
            commands::handle_energy_monthly(&host, year, outlet).await
        }
        Command::Specs { host } => admin::handle_specs(&host).await,
        Command::Presets { host } => admin::handle_presets(&host).await,
        Command::Effects { host } => effects::handle_effects_command(&host).await,
        Command::Effect { host, name } => effects::handle_effect_command(&host, &name).await,
        Command::Schedules { host } => admin::handle_schedules(&host).await,
        Command::Led { host, state } => {
            let on = matches!(state, crate::cli::LedAction::On);
            admin::handle_led(&host, on).await
        }
        Command::Clock { host } => admin::handle_clock(&host).await,
        Command::Rename { host, name } => admin::handle_rename(&host, &name).await,
        Command::Restart { host } => admin::handle_restart(&host).await,
        Command::Outlets { host } => admin::handle_outlets(&host).await,
        Command::OutletRename { host, outlet, name } => {
            admin::handle_outlet_rename(&host, outlet, &name).await
        }
        Command::Alias { name, ip, klap } => admin::handle_alias(&name, &ip, klap),
        Command::Unalias { name } => admin::handle_unalias(&name),
        Command::Aliases => admin::handle_aliases(),
        Command::Login { email, password } => admin::handle_login(&email, password),
        Command::Completions { shell } => {
            admin::handle_completions(shell);
            Ok(())
        }
    }
}

pub async fn run() -> Result<()> {
    let cli = Cli::parse();
    dispatch_command(cli.command).await
}
