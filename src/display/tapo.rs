use super::common::{
    header, on_state, on_state_detail, short_fw, signal_summary, tapo_signal_label,
};
use crate::tapo::TapoDevice;
use colored::Colorize;

pub fn print_unknown_summary(ip: std::net::IpAddr, json: &serde_json::Value, type_str: &str) {
    let alias = json
        .pointer("/system/get_sysinfo/alias")
        .and_then(|v| v.as_str())
        .unwrap_or("(unnamed)");
    let model = json
        .pointer("/system/get_sysinfo/model")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    println!(
        "{} {} [{}]",
        header(alias),
        format!("[{ip}]").dimmed(),
        type_str.dimmed(),
    );
    println!("   {} {}", model, "— unsupported device type".dimmed());
    println!();
}

pub fn print_tapo_summary(ip: &str, d: &TapoDevice, hint_alias: &str) {
    println!(
        "{} {} {} {} {}",
        header(hint_alias),
        "[tapo]".dimmed(),
        format!("[{ip}]").dimmed(),
        on_state(d.is_on()),
        signal_summary(d.rssi),
    );
    println!("   {} HW:{}  FW:{}", d.model, d.hw_ver, short_fw(&d.fw_ver));
    if d.is_on() && d.on_time > 0 {
        println!("   On for: {}", crate::fmt::duration(d.on_time));
    }
    let action = if d.is_on() { "off" } else { "on" };
    println!(
        "   {}",
        format!("→ denki {action} \"{hint_alias}\"").dimmed()
    );
    println!();
}

pub fn print_tapo_detail(ip: &str, d: &TapoDevice, hint_alias: &str) {
    println!("{}", header(hint_alias));
    println!("  Host:      {ip}");
    println!("  State:     {}", on_state_detail(d.is_on()));
    println!("  Model:     {}", d.model);
    println!("  Hardware:  {}", d.hw_ver);
    println!("  Firmware:  {}", d.fw_ver);
    println!(
        "  Signal:    {} dBm  {}",
        d.rssi,
        tapo_signal_label(d.signal_level)
    );
    if d.is_on() && d.on_time > 0 {
        println!("  On for:    {}", crate::fmt::duration(d.on_time));
    }
    if d.overheated {
        println!("  {}", "WARNING: device overheated".red().bold());
    }
    let action = if d.is_on() { "off" } else { "on" };
    println!(
        "  {}",
        format!("→ denki {action} \"{hint_alias}\"").dimmed()
    );
}
