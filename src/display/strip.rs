use super::common::{
    header, on_state, on_state_detail, print_light_color, print_light_effect_detail,
    print_light_state_detail, short_fw, signal_label, signal_summary,
};
use super::hints::{dimmer_hints, lightstrip_hints, plug_hints, strip_hints};
use crate::bulb::Bulb;
use crate::dimmer::Dimmer;
use crate::plug::Plug;
use crate::strip::Strip;
use colored::Colorize;
use std::net::IpAddr;

// Light strips share the Bulb struct but use the smartlife.iot.lightStrip namespace.
pub fn print_lightstrip_summary(ip: IpAddr, bulb: &Bulb, hint_alias: &str) {
    crate::output::println!(
        "{} {} {} {} {}",
        header(&bulb.alias),
        "[light strip]".dimmed(),
        format!("[{ip}]").dimmed(),
        on_state(bulb.light_state.is_on()),
        signal_summary(bulb.rssi),
    );
    crate::output::println!(
        "   {} HW:{}  FW:{}",
        bulb.model,
        bulb.hw_ver,
        short_fw(&bulb.sw_ver)
    );
    print_light_color(&bulb.light_state, "   ");
    crate::output::println!(
        "   {}",
        format!("→ {}", lightstrip_hints(bulb, hint_alias).join("  ·  ")).dimmed()
    );
    crate::output::println!();
}

pub fn print_lightstrip_detail(ip: &str, bulb: &Bulb, hint_alias: &str) {
    crate::output::println!("{} {}", header(&bulb.alias), "[light strip]".dimmed());
    crate::output::println!("  Host:       {ip}");
    crate::output::println!(
        "  State:      {}",
        on_state_detail(bulb.light_state.is_on())
    );
    crate::output::println!("  Model:      {}", bulb.model);
    crate::output::println!("  Hardware:   {}", bulb.hw_ver);
    crate::output::println!("  Firmware:   {}", bulb.sw_ver);
    crate::output::println!(
        "  Signal:     {} dBm  {}",
        bulb.rssi,
        signal_label(bulb.rssi)
    );
    print_light_state_detail(&bulb.light_state);
    print_light_effect_detail(bulb.lighting_effect_state.as_ref());
    crate::output::println!(
        "  {}",
        format!("→ {}", lightstrip_hints(bulb, hint_alias).join("  ·  ")).dimmed()
    );
    crate::output::println!(
        "  {}",
        "NOTE: unverified — not tested on live hardware".yellow()
    );
}

pub fn print_dimmer_summary(ip: IpAddr, d: &Dimmer, hint_alias: &str) {
    crate::output::println!(
        "{} {} {} {} {}",
        header(&d.alias),
        "[dimmer]".dimmed(),
        format!("[{ip}]").dimmed(),
        on_state(d.is_on()),
        signal_summary(d.rssi),
    );
    crate::output::println!(
        "   {} HW:{}  FW:{}  {}%",
        d.model,
        d.hw_ver,
        short_fw(&d.sw_ver),
        d.brightness
    );
    crate::output::println!(
        "   {}",
        format!("→ {}", dimmer_hints(d, hint_alias).join("  ·  ")).dimmed()
    );
    crate::output::println!();
}

pub fn print_dimmer_detail(ip: &str, d: &Dimmer, hint_alias: &str) {
    crate::output::println!("{} {}", header(&d.alias), "[dimmer]".dimmed());
    crate::output::println!("  Host:       {ip}");
    crate::output::println!("  State:      {}", on_state_detail(d.is_on()));
    crate::output::println!("  Model:      {}", d.model);
    crate::output::println!("  Hardware:   {}", d.hw_ver);
    crate::output::println!("  Firmware:   {}", d.sw_ver);
    crate::output::println!("  Signal:     {} dBm  {}", d.rssi, signal_label(d.rssi));
    crate::output::println!("  Brightness: {}%", d.brightness);
    crate::output::println!(
        "  {}",
        format!("→ {}", dimmer_hints(d, hint_alias).join("  ·  ")).dimmed()
    );
    crate::output::println!(
        "  {}",
        "NOTE: unverified — not tested on live hardware".yellow()
    );
}

pub fn print_plug_summary(ip: IpAddr, plug: &Plug, hint_alias: &str) {
    let energy_tag = if plug.has_energy_monitoring() {
        "  energy".dimmed()
    } else {
        "".normal()
    };
    crate::output::println!(
        "{} {} {} {}{}",
        header(&plug.alias),
        format!("[{ip}]").dimmed(),
        on_state(plug.is_on()),
        signal_summary(plug.rssi),
        energy_tag,
    );
    crate::output::println!(
        "   {} HW:{}  FW:{}",
        plug.model,
        plug.hw_ver,
        short_fw(&plug.sw_ver)
    );
    if plug.is_on() {
        crate::output::println!("   On for: {}", plug.on_time_fmt());
    }
    crate::output::println!(
        "   {}",
        format!("→ {}", plug_hints(plug, hint_alias).join("  ·  ")).dimmed()
    );
    crate::output::println!();
}

pub fn print_plug_detail(ip: &str, plug: &Plug, hint_alias: &str) {
    crate::output::println!("{}", header(&plug.alias));
    crate::output::println!("  Host:     {ip}");
    crate::output::println!("  State:    {}", on_state_detail(plug.is_on()));
    crate::output::println!("  Model:    {}", plug.model);
    crate::output::println!("  Hardware: {}", plug.hw_ver);
    crate::output::println!("  Firmware: {}", plug.sw_ver);
    crate::output::println!("  Signal:   {} dBm  {}", plug.rssi, signal_label(plug.rssi));
    crate::output::println!(
        "  LED:      {}",
        if plug.led_off == 1 { "off" } else { "on" }
    );
    if plug.is_on() {
        crate::output::println!("  On for:   {}", plug.on_time_fmt());
    }
    crate::output::println!(
        "  {}",
        format!("→ {}", plug_hints(plug, hint_alias).join("  ·  ")).dimmed()
    );
}

pub fn print_strip_summary(ip: IpAddr, s: &Strip, hint_alias: &str) {
    let on_count = s.children.iter().filter(|c| c.is_on()).count();
    let total = s.children.len();
    let state = if on_count > 0 {
        format!("{on_count}/{total} on").green().bold()
    } else {
        "all off".dimmed()
    };
    let energy_tag = if s.has_energy_monitoring() {
        "  energy".dimmed()
    } else {
        "".normal()
    };
    crate::output::println!(
        "{} {} {} {} {}{}",
        header(&s.alias),
        "[strip]".dimmed(),
        format!("[{ip}]").dimmed(),
        state,
        signal_summary(s.rssi),
        energy_tag,
    );
    crate::output::println!("   {} HW:{}  FW:{}", s.model, s.hw_ver, short_fw(&s.sw_ver));
    let outlet_line = s
        .children
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let label = format!("{} {}", i + 1, c.alias);
            if c.is_on() {
                label.green().bold().to_string()
            } else {
                label.dimmed().to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("  ");
    crate::output::println!("   {outlet_line}");
    crate::output::println!(
        "   {}",
        format!("→ {}", strip_hints(s, hint_alias).join("  ·  ")).dimmed()
    );
    crate::output::println!();
}

pub fn print_strip_detail(ip: &str, s: &Strip, hint_alias: &str) {
    let on_count = s.children.iter().filter(|c| c.is_on()).count();
    crate::output::println!("{} {}", header(&s.alias), "[strip]".dimmed());
    crate::output::println!("  Host:     {ip}");
    crate::output::println!("  Model:    {}", s.model);
    crate::output::println!("  Hardware: {}", s.hw_ver);
    crate::output::println!("  Firmware: {}", s.sw_ver);
    crate::output::println!("  Signal:   {} dBm  {}", s.rssi, signal_label(s.rssi));
    crate::output::println!("  Outlets:  {}/{} on", on_count, s.children.len());
    print_strip_outlets(s);
    crate::output::println!(
        "  {}",
        format!("→ {}", strip_hints(s, hint_alias).join("  ·  ")).dimmed()
    );
    let verified = crate::devices::lookup(&s.model).is_some_and(|e| e.verified);
    if !verified {
        crate::output::println!(
            "  {}",
            "NOTE: unverified — not tested on live hardware".yellow()
        );
    }
}

pub fn print_strip_outlets(s: &Strip) {
    for (i, child) in s.children.iter().enumerate() {
        let n = i + 1;
        let on_time = if child.is_on() && child.on_time > 0 {
            format!("  (on for {})", child.on_time_fmt())
                .dimmed()
                .to_string()
        } else {
            String::new()
        };
        crate::output::println!(
            "  Outlet {n}: {}  {}{}",
            on_state(child.is_on()),
            child.alias,
            on_time
        );
    }
}
