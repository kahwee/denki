use super::common::{format_energy_lines, format_wday, sort_energy_entries};
use crate::tapo::TapoEnergyUsage;
use colored::Colorize;

pub fn print_tapo_energy(usage: &TapoEnergyUsage) {
    if let Some(power) = usage.current_power {
        println!("Power:         {:.3} W", power as f64 / 1000.0);
    }
    if let Some(today) = usage.today_energy {
        println!("Today:         {today} Wh");
    }
    if let Some(month) = usage.month_energy {
        println!("This month:    {month} Wh");
    }
    if let Some(minutes) = usage.today_runtime {
        println!("Runtime today: {}m", minutes);
    }
    if usage.current_power.is_none() && usage.today_energy.is_none() && usage.month_energy.is_none()
    {
        println!(
            "{}",
            "No energy measurements were returned by this Tapo device.".yellow()
        );
    }
}

pub fn print_schedules(json: &serde_json::Value) {
    if let Some(rules) = json
        .pointer("/schedule/get_rules/rule_list")
        .and_then(|v| v.as_array())
    {
        if rules.is_empty() {
            println!("No schedules configured.");
            return;
        }
        println!("{}", "Schedules:".bold());
        for r in rules {
            let enabled = r
                .get("enable")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
                == 1;
            let name = r
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("(unnamed)");
            let smin = r
                .get("smin")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            let sact = r
                .get("sact")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(1);
            let action = if sact == 1 { "on".green() } else { "off".red() };
            let time = format!("{:02}:{:02}", smin / 60, smin % 60);
            let days = format_wday(r.get("wday").and_then(|v| v.as_array()));
            let status = if enabled {
                "".normal()
            } else {
                " (disabled)".dimmed()
            };
            println!(
                "  {} at {}  {}  {}{}",
                action,
                time,
                days,
                name.bold(),
                status
            );
        }
    }
}

pub fn print_energy_realtime(json: &serde_json::Value) {
    let data = json
        .pointer("/emeter/get_realtime")
        .or_else(|| json.pointer("/smartlife.iot.common.emeter/get_realtime"));

    if let Some(d) = data {
        if d.get("err_code")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0)
            != 0
        {
            let msg = d
                .get("err_msg")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            println!("{}", format!("Energy not supported: {msg}").yellow());
            return;
        }
        for line in format_energy_lines(d) {
            println!("{line}");
        }
    } else {
        println!(
            "{}",
            "No energy data — device may not support energy monitoring.".yellow()
        );
    }
}

pub fn print_energy_daily(json: &serde_json::Value, month: &str) {
    let days = json
        .pointer("/emeter/get_daystat/day_list")
        .or_else(|| json.pointer("/smartlife.iot.common.emeter/get_daystat/day_list"))
        .and_then(|v| v.as_array());

    if let Some(list) = days {
        println!("{}", format!("Daily energy usage for {month}:").bold());
        for (day, wh) in sort_energy_entries(list, "day") {
            let bar = "#".repeat((wh / 10).min(40) as usize);
            println!("  Day {:2}: {:4} Wh  {}", day, wh, bar.yellow());
        }
    }
}

pub fn print_energy_monthly(json: &serde_json::Value, year: u16) {
    let months = json
        .pointer("/emeter/get_monthstat/month_list")
        .or_else(|| json.pointer("/smartlife.iot.common.emeter/get_monthstat/month_list"))
        .and_then(|v| v.as_array());

    if let Some(list) = months {
        println!("{}", format!("Monthly energy usage for {year}:").bold());
        for (month, wh) in sort_energy_entries(list, "month") {
            let bar = "#".repeat((wh / 100).min(40) as usize);
            println!("  Month {:2}: {:5} Wh  {}", month, wh, bar.yellow());
        }
    }
}
