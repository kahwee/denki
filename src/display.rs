use crate::bulb::{Bulb, LightState, LightingEffectState};
use crate::devices;
use crate::dimmer::Dimmer;
use crate::plug::Plug;
use crate::strip::Strip;
use crate::tapo::TapoDevice;
use colored::{ColoredString, Colorize};
use std::net::IpAddr;

fn on_state(is_on: bool) -> ColoredString {
    if is_on {
        "on".green().bold()
    } else {
        "off".dimmed()
    }
}

fn on_state_detail(is_on: bool) -> ColoredString {
    if is_on {
        "ON".green().bold()
    } else {
        "OFF".red()
    }
}

fn header(name: &str) -> ColoredString {
    format!("== {name} ==").bold()
}

/// Trim "1.1.1 Build 250908 Rel.112945" → "1.1.1".
fn short_fw(fw: &str) -> &str {
    fw.split_whitespace().next().unwrap_or(fw)
}

/// Energy entries may expose `energy_wh` (integer Wh) or `energy` (float kWh).
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn wh_from(entry: &serde_json::Value) -> u64 {
    entry
        .get("energy_wh")
        .and_then(serde_json::Value::as_u64)
        .or_else(|| {
            entry
                .get("energy")
                .and_then(serde_json::Value::as_f64)
                .map(|kwh| (kwh * 1000.0).round() as u64)
        })
        .unwrap_or(0)
}

/// Convert HSV (h: 0–360, s: 0–100, v: 0–100) to (r, g, b) each 0–255.
#[allow(
    clippy::many_single_char_names,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn hsv_to_rgb(h: u16, s: u8, v: u8) -> (u8, u8, u8) {
    let s = f32::from(s) / 100.0;
    let v = f32::from(v) / 100.0;
    let h = f32::from(h);
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r1, g1, b1) = match (h as u16) / 60 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (
        ((r1 + m) * 255.0) as u8,
        ((g1 + m) * 255.0) as u8,
        ((b1 + m) * 255.0) as u8,
    )
}

fn print_light_color(ls: &LightState, indent: &str) {
    if ls.color_temp() > 0 {
        println!(
            "{indent}Brightness: {}%  Warmth: {}K",
            ls.brightness(),
            ls.color_temp()
        );
    } else {
        let (r, g, b) = hsv_to_rgb(ls.hue(), ls.saturation(), ls.brightness());
        let swatch = "██".truecolor(r, g, b);
        println!(
            "{indent}Brightness: {}%  Color: {} {}° hue  {} sat",
            ls.brightness(),
            swatch,
            ls.hue(),
            ls.saturation(),
        );
    }
}

pub fn print_bulb_summary(ip: IpAddr, bulb: &Bulb, hint_alias: &str) {
    println!(
        "{} {} {} {}",
        header(&bulb.alias),
        format!("[{ip}]").dimmed(),
        on_state(bulb.light_state.is_on()),
        signal_summary(bulb.rssi),
    );
    println!(
        "   {} {}  HW:{}  FW:{}",
        bulb.model,
        caps_label(bulb).dimmed(),
        bulb.hw_ver,
        short_fw(&bulb.sw_ver),
    );
    print_light_color(&bulb.light_state, "   ");
    println!(
        "   {}",
        format!("→ {}", bulb_hints(bulb, hint_alias).join("  ·  ")).dimmed()
    );
    println!();
}

/// Print brightness + color-temperature or HSV color for any light state.
/// Used by both bulb and light-strip detail views.
fn print_light_state_detail(ls: &LightState) {
    println!("  Brightness: {}%", ls.brightness());
    if ls.color_temp() > 0 {
        println!("  Warmth:     {}K", ls.color_temp());
    } else {
        let (r, g, b) = hsv_to_rgb(ls.hue(), ls.saturation(), ls.brightness());
        println!(
            "  Color:      {} {}° hue  {} sat",
            "██".truecolor(r, g, b),
            ls.hue(),
            ls.saturation()
        );
    }
}

fn print_light_effect_detail(effect: Option<&LightingEffectState>) {
    if let Some(effect) = effect {
        let name = if effect.enable == 1 {
            effect.name.as_str()
        } else {
            "Off"
        };
        println!("  Effect:     {}", name);
    }
}

pub fn print_bulb_detail(ip: &str, bulb: &Bulb, hint_alias: &str) {
    println!("{}", header(&bulb.alias));
    println!("  Host:       {ip}");
    println!(
        "  State:      {}",
        on_state_detail(bulb.light_state.is_on())
    );
    println!("  Model:      {}", bulb.model);
    println!("  Hardware:   {}", bulb.hw_ver);
    println!("  Firmware:   {}", bulb.sw_ver);
    println!(
        "  Signal:     {} dBm  {}",
        bulb.rssi,
        signal_label(bulb.rssi)
    );
    print_light_state_detail(&bulb.light_state);
    print_light_effect_detail(bulb.lighting_effect_state.as_ref());
    println!("  Features:   {}", caps_label(bulb));
    println!(
        "  {}",
        format!("→ {}", bulb_hints(bulb, hint_alias).join("  ·  ")).dimmed()
    );
}

pub fn print_bulb_specs(json: &serde_json::Value) {
    if let Some(s) = json.pointer("/smartlife.iot.smartbulb.lightingservice/get_light_details") {
        println!(
            "Beam angle:              {}°",
            s["lamp_beam_angle"].as_u64().unwrap_or(0)
        );
        println!(
            "Wattage:                 {}W",
            s["wattage"].as_u64().unwrap_or(0)
        );
        println!(
            "Incandescent equivalent: {}W",
            s["incandescent_equivalent"].as_u64().unwrap_or(0)
        );
        println!(
            "Max lumens:              {}",
            s["max_lumens"].as_u64().unwrap_or(0)
        );
        println!(
            "Color rendering index:   {}",
            s["color_rendering_index"].as_u64().unwrap_or(0)
        );
        println!(
            "Voltage range:           {}-{}V",
            s["min_voltage"].as_u64().unwrap_or(0),
            s["max_voltage"].as_u64().unwrap_or(0)
        );
    }
}

pub fn print_bulb_presets(json: &serde_json::Value) {
    if let Some(states) = json
        .pointer("/smartlife.iot.smartbulb.lightingservice/get_preferred_state/states")
        .and_then(|v| v.as_array())
    {
        println!("{}", "Saved presets:".bold());
        for s in states {
            let idx = s["index"].as_u64().unwrap_or(0) + 1;
            let brightness = s["brightness"].as_u64().unwrap_or(0);
            let color_temp = s["color_temp"].as_u64().unwrap_or(0);
            let hue = s["hue"].as_u64().unwrap_or(0);
            let sat = s["saturation"].as_u64().unwrap_or(0);
            if color_temp > 0 {
                println!("  Preset {idx}: {brightness}% brightness  {color_temp}K");
            } else {
                println!("  Preset {idx}: {brightness}% brightness  hue={hue} sat={sat}");
            }
        }
    }
}

pub fn print_plug_summary(ip: IpAddr, plug: &Plug, hint_alias: &str) {
    let energy_tag = if plug.has_energy_monitoring() {
        "  energy".dimmed()
    } else {
        "".normal()
    };
    println!(
        "{} {} {} {}{}",
        header(&plug.alias),
        format!("[{ip}]").dimmed(),
        on_state(plug.is_on()),
        signal_summary(plug.rssi),
        energy_tag,
    );
    println!(
        "   {} HW:{}  FW:{}",
        plug.model,
        plug.hw_ver,
        short_fw(&plug.sw_ver)
    );
    if plug.is_on() {
        println!("   On for: {}", plug.on_time_fmt());
    }
    println!(
        "   {}",
        format!("→ {}", plug_hints(plug, hint_alias).join("  ·  ")).dimmed()
    );
    println!();
}

pub fn print_plug_detail(ip: &str, plug: &Plug, hint_alias: &str) {
    println!("{}", header(&plug.alias));
    println!("  Host:     {ip}");
    println!("  State:    {}", on_state_detail(plug.is_on()));
    println!("  Model:    {}", plug.model);
    println!("  Hardware: {}", plug.hw_ver);
    println!("  Firmware: {}", plug.sw_ver);
    println!("  Signal:   {} dBm  {}", plug.rssi, signal_label(plug.rssi));
    println!(
        "  LED:      {}",
        if plug.led_off == 1 { "off" } else { "on" }
    );
    if plug.is_on() {
        println!("  On for:   {}", plug.on_time_fmt());
    }
    println!(
        "  {}",
        format!("→ {}", plug_hints(plug, hint_alias).join("  ·  ")).dimmed()
    );
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

fn format_wday(wday: Option<&Vec<serde_json::Value>>) -> String {
    const LABELS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    let Some(days) = wday else {
        return "every day".to_string();
    };
    let active: Vec<&str> = days
        .iter()
        .enumerate()
        .filter(|(_, v)| v.as_u64().unwrap_or(0) == 1)
        .map(|(i, _)| LABELS[i])
        .collect();
    match active.len() {
        0 => "no days".to_string(),
        7 => "every day".to_string(),
        _ => active.join(" "),
    }
}

/// Extract energy fields into plain strings for testing and display.
/// Returns lines like "Power:   5.40 W" without color.
fn format_energy_lines(d: &serde_json::Value) -> Vec<String> {
    let mut lines = Vec::new();
    if d.get("power_mw").is_some() {
        // KP115 / color-bulb milli-unit fields (KL135, LB130)
        if let Some(mw) = d.get("power_mw").and_then(serde_json::Value::as_f64) {
            lines.push(format!("Power:   {:.2} W", mw / 1000.0));
        }
        if let Some(mv) = d.get("voltage_mv").and_then(serde_json::Value::as_f64) {
            lines.push(format!("Voltage: {:.1} V", mv / 1000.0));
        }
        if let Some(ma) = d.get("current_ma").and_then(serde_json::Value::as_f64) {
            lines.push(format!("Current: {:.3} A", ma / 1000.0));
        }
        if let Some(wh) = d.get("total_wh").and_then(serde_json::Value::as_u64) {
            lines.push(format!("Total:   {wh} Wh"));
        }
    } else {
        // HS110 real-unit fields
        if let Some(w) = d.get("power").and_then(serde_json::Value::as_f64) {
            lines.push(format!("Power:   {w:.2} W"));
        }
        if let Some(v) = d.get("voltage").and_then(serde_json::Value::as_f64) {
            lines.push(format!("Voltage: {v:.1} V"));
        }
        if let Some(a) = d.get("current").and_then(serde_json::Value::as_f64) {
            lines.push(format!("Current: {a:.3} A"));
        }
        if let Some(kwh) = d.get("total").and_then(serde_json::Value::as_f64) {
            lines.push(format!("Total:   {kwh:.3} kWh"));
        }
    }
    lines
}

pub fn print_energy_realtime(json: &serde_json::Value) {
    // Three possible paths:
    //   KL135/LB130: /smartlife.iot.common.emeter/get_realtime — power_mw + total_wh only
    //   KP115: /emeter/get_realtime — milli-unit fields (mv, ma, mw, wh)
    //   HS110: /emeter/get_realtime — real-unit fields (V, A, W, kWh)
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

fn signal_label(rssi: i32) -> colored::ColoredString {
    if rssi >= -50 {
        "excellent".green()
    } else if rssi >= -65 {
        "good".yellow()
    } else {
        "weak".red()
    }
}

fn signal_summary(rssi: i32) -> String {
    format!("signal:{}", signal_label(rssi))
}

fn caps_label(bulb: &Bulb) -> String {
    let mut caps = vec![];
    if bulb.is_color == 1 {
        caps.push("color");
    }
    if bulb.is_variable_color_temp == 1 {
        caps.push("color-temp");
    }
    if bulb.is_dimmable == 1 {
        caps.push("dimmable");
    }
    caps.join(", ")
}

// ── Sorting helpers ───────────────────────────────────────────────────────────

/// Extract and sort energy entries by `key` (e.g. "day" or "month"). Returns (key, Wh) pairs.
fn sort_energy_entries(list: &[serde_json::Value], key: &str) -> Vec<(u64, u64)> {
    let mut entries: Vec<(u64, u64)> = list
        .iter()
        .map(|e| (e[key].as_u64().unwrap_or(0), wh_from(e)))
        .collect();
    entries.sort_by_key(|(k, _)| *k);
    entries
}

// ── Hint builders (shared between summary and detail views) ──────────────────

/// Registry-based hints, falling back to a bare on/off hint for unknown models.
fn model_hints(model: &str, alias: &str, is_on: bool) -> Vec<String> {
    devices::lookup(model).map_or_else(
        || {
            let action = if is_on { "off" } else { "on" };
            vec![format!("denki {action} \"{alias}\"")]
        },
        |e| devices::hints(e, alias, is_on),
    )
}

fn bulb_hints(bulb: &Bulb, alias: &str) -> Vec<String> {
    model_hints(&bulb.model, alias, bulb.light_state.is_on())
}

fn plug_hints(plug: &Plug, alias: &str) -> Vec<String> {
    // Runtime ENE flag overrides devices.toml: drop energy hint if chip absent.
    let mut h = model_hints(&plug.model, alias, plug.is_on());
    if !plug.has_energy_monitoring() {
        h.retain(|s| !s.contains("energy"));
    }
    h
}

fn dimmer_hints(d: &Dimmer, alias: &str) -> Vec<String> {
    model_hints(&d.model, alias, d.is_on())
}

fn lightstrip_hints(bulb: &Bulb, alias: &str) -> Vec<String> {
    devices::lookup(&bulb.model)
        .map(|e| {
            let mut h = devices::hints(e, alias, bulb.light_state.is_on());
            // Remove the unconditional on/off hint when power is not yet implemented
            if !e.supports.iter().any(|f| f == "power") {
                h.remove(0);
            }
            if e.supports.iter().any(|f| f == "energy") {
                h.push(format!("denki energy-daily \"{alias}\""));
                h.push(format!("denki energy-monthly \"{alias}\""));
            }
            h
        })
        .unwrap_or_default()
}

fn strip_hints(s: &Strip, alias: &str) -> Vec<String> {
    let mut hints = devices::lookup(&s.model)
        .map(|e| {
            let mut h = devices::hints(
                e,
                alias,
                s.children.iter().any(crate::strip::StripChild::is_on),
            );
            if !s.has_energy_monitoring() {
                h.retain(|h| !h.contains("energy"));
            }
            h
        })
        .unwrap_or_default();
    hints.push(format!("denki on \"{alias}\" 1"));
    hints.push(format!("denki off \"{alias}\" 1"));
    hints.push(format!("denki outlet-rename \"{alias}\" 1 \"Name\""));
    if s.has_energy_monitoring() {
        hints.push(format!("denki energy \"{alias}\" 1"));
    }
    hints
}

// Light strips share the Bulb struct but use the smartlife.iot.lightStrip namespace.
pub fn print_lightstrip_summary(ip: IpAddr, bulb: &Bulb, hint_alias: &str) {
    println!(
        "{} {} {} {} {}",
        header(&bulb.alias),
        "[light strip]".dimmed(),
        format!("[{ip}]").dimmed(),
        on_state(bulb.light_state.is_on()),
        signal_summary(bulb.rssi),
    );
    println!(
        "   {} HW:{}  FW:{}",
        bulb.model,
        bulb.hw_ver,
        short_fw(&bulb.sw_ver)
    );
    print_light_color(&bulb.light_state, "   ");
    println!(
        "   {}",
        format!("→ {}", lightstrip_hints(bulb, hint_alias).join("  ·  ")).dimmed()
    );
    println!();
}

pub fn print_lightstrip_detail(ip: &str, bulb: &Bulb, hint_alias: &str) {
    println!("{} {}", header(&bulb.alias), "[light strip]".dimmed());
    println!("  Host:       {ip}");
    println!(
        "  State:      {}",
        on_state_detail(bulb.light_state.is_on())
    );
    println!("  Model:      {}", bulb.model);
    println!("  Hardware:   {}", bulb.hw_ver);
    println!("  Firmware:   {}", bulb.sw_ver);
    println!(
        "  Signal:     {} dBm  {}",
        bulb.rssi,
        signal_label(bulb.rssi)
    );
    print_light_state_detail(&bulb.light_state);
    print_light_effect_detail(bulb.lighting_effect_state.as_ref());
    println!(
        "  {}",
        format!("→ {}", lightstrip_hints(bulb, hint_alias).join("  ·  ")).dimmed()
    );
    println!(
        "  {}",
        "NOTE: unverified — not tested on live hardware".yellow()
    );
}

pub fn print_dimmer_summary(ip: IpAddr, d: &Dimmer, hint_alias: &str) {
    println!(
        "{} {} {} {} {}",
        header(&d.alias),
        "[dimmer]".dimmed(),
        format!("[{ip}]").dimmed(),
        on_state(d.is_on()),
        signal_summary(d.rssi),
    );
    println!(
        "   {} HW:{}  FW:{}  {}%",
        d.model,
        d.hw_ver,
        short_fw(&d.sw_ver),
        d.brightness
    );
    println!(
        "   {}",
        format!("→ {}", dimmer_hints(d, hint_alias).join("  ·  ")).dimmed()
    );
    println!();
}

pub fn print_dimmer_detail(ip: &str, d: &Dimmer, hint_alias: &str) {
    println!("{} {}", header(&d.alias), "[dimmer]".dimmed());
    println!("  Host:       {ip}");
    println!("  State:      {}", on_state_detail(d.is_on()));
    println!("  Model:      {}", d.model);
    println!("  Hardware:   {}", d.hw_ver);
    println!("  Firmware:   {}", d.sw_ver);
    println!("  Signal:     {} dBm  {}", d.rssi, signal_label(d.rssi));
    println!("  Brightness: {}%", d.brightness);
    println!(
        "  {}",
        format!("→ {}", dimmer_hints(d, hint_alias).join("  ·  ")).dimmed()
    );
    println!(
        "  {}",
        "NOTE: unverified — not tested on live hardware".yellow()
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
    println!(
        "{} {} {} {} {}{}",
        header(&s.alias),
        "[strip]".dimmed(),
        format!("[{ip}]").dimmed(),
        state,
        signal_summary(s.rssi),
        energy_tag,
    );
    println!("   {} HW:{}  FW:{}", s.model, s.hw_ver, short_fw(&s.sw_ver));
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
    println!("   {outlet_line}");
    let a = hint_alias;
    println!(
        "   {}",
        format!("→ {}", strip_hints(s, a).join("  ·  ")).dimmed()
    );
    println!();
}

pub fn print_strip_detail(ip: &str, s: &Strip, hint_alias: &str) {
    let on_count = s.children.iter().filter(|c| c.is_on()).count();
    println!("{} {}", header(&s.alias), "[strip]".dimmed());
    println!("  Host:     {ip}");
    println!("  Model:    {}", s.model);
    println!("  Hardware: {}", s.hw_ver);
    println!("  Firmware: {}", s.sw_ver);
    println!("  Signal:   {} dBm  {}", s.rssi, signal_label(s.rssi));
    println!("  Outlets:  {}/{} on", on_count, s.children.len());
    print_strip_outlets(s);
    let a = hint_alias;
    println!(
        "  {}",
        format!("→ {}", strip_hints(s, a).join("  ·  ")).dimmed()
    );
    let verified = devices::lookup(&s.model).is_some_and(|e| e.verified);
    if !verified {
        println!(
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
        println!(
            "  Outlet {n}: {}  {}{}",
            on_state(child.is_on()),
            child.alias,
            on_time
        );
    }
}

pub fn print_unknown_summary(ip: IpAddr, json: &serde_json::Value, type_str: &str) {
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

fn tapo_signal_label(level: u8) -> colored::ColoredString {
    match level {
        3 => "excellent".green(),
        2 => "good".yellow(),
        1 => "weak".red(),
        _ => "no signal".red(),
    }
}

#[cfg(test)]
mod tests;
