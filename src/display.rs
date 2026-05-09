use crate::bulb::{Bulb, LightState};
use crate::dimmer::Dimmer;
use crate::plug::Plug;
use crate::strip::Strip;
use crate::tapo::TapoDevice;
use colored::{ColoredString, Colorize};
use std::net::IpAddr;

// ── Private helpers ───────────────────────────────────────────────────────────

fn on_state(is_on: bool) -> ColoredString {
    if is_on { "on".green().bold() } else { "off".dimmed() }
}

fn on_state_detail(is_on: bool) -> ColoredString {
    if is_on { "ON".green().bold() } else { "OFF".red() }
}

fn header(name: &str) -> ColoredString {
    format!("== {name} ==").bold()
}

/// Trim firmware strings like "1.1.1 Build 250908 Rel.112945" to just "1.1.1".
fn short_fw(fw: &str) -> &str {
    fw.split_whitespace().next().unwrap_or(fw)
}

/// Extract Wh from a day/month energy entry.
/// KP115/KL135 use `energy_wh` (integer Wh); HS110 uses `energy` (float kWh).
fn wh_from(entry: &serde_json::Value) -> u64 {
    entry
        .get("energy_wh")
        .and_then(|v| v.as_u64())
        .or_else(|| {
            entry
                .get("energy")
                .and_then(|v| v.as_f64())
                .map(|kwh| (kwh * 1000.0) as u64)
        })
        .unwrap_or(0)
}

/// Print the color/warmth line for a light state.
fn print_light_color(ls: &LightState, indent: &str) {
    if ls.color_temp() > 0 {
        println!("{indent}Brightness: {}%  Warmth: {}K", ls.brightness(), ls.color_temp());
    } else {
        // val == brightness in HSV — skip the redundant field, format hue with degree symbol
        println!(
            "{indent}Brightness: {}%  Color: {}° hue  {} sat",
            ls.brightness(),
            ls.hue(),
            ls.saturation(),
        );
    }
}

// ── Bulb display ─────────────────────────────────────────────────────────────

pub fn print_bulb_summary(ip: IpAddr, bulb: &Bulb) {
    println!(
        "{} {} {} {}",
        header(&bulb.alias),
        format!("[{ip}]").dimmed(),
        on_state(bulb.light_state.is_on()),
        signal_label(bulb.rssi),
    );
    println!(
        "   {} {}  HW:{}  FW:{}",
        bulb.model,
        caps_label(bulb).dimmed(),
        bulb.hw_ver,
        short_fw(&bulb.sw_ver),
    );
    print_light_color(&bulb.light_state, "   ");
    let a = &bulb.alias;
    let action = if bulb.light_state.is_on() { "off" } else { "on" };
    let color_hint = if bulb.is_color == 1 {
        format!("  ·  denki warmth \"{a}\" 2700  ·  denki dim \"{a}\" 80")
    } else if bulb.is_dimmable == 1 {
        format!("  ·  denki dim \"{a}\" 80")
    } else {
        String::new()
    };
    println!("   {}", format!("→ denki power \"{a}\" {action}{color_hint}").dimmed());
    println!();
}

pub fn print_bulb_detail(ip: &str, bulb: &Bulb) {
    println!("{}", header(&bulb.alias));
    println!("  Host:       {ip}");
    println!("  State:      {}", on_state_detail(bulb.light_state.is_on()));
    println!("  Model:      {}", bulb.model);
    println!("  Hardware:   {}", bulb.hw_ver);
    println!("  Firmware:   {}", bulb.sw_ver);
    println!("  Signal:     {} dBm  {}", bulb.rssi, signal_label(bulb.rssi));
    let ls = &bulb.light_state;
    println!("  Brightness: {}%", ls.brightness());
    if ls.color_temp() > 0 {
        println!("  Warmth:     {}K", ls.color_temp());
    } else {
        println!("  Color:      {}° hue  {} sat", ls.hue(), ls.saturation());
    }
    println!("  Features:   {}", caps_label(bulb));
    println!("  {}", "Energy:     use `denki energy <host>`".dimmed());
    println!("  {}", "Specs:      use `denki specs <host>`".dimmed());
    println!("  {}", "Presets:    use `denki presets <host>`".dimmed());
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
                println!(
                    "  Preset {idx}: {}% brightness  {}K",
                    brightness, color_temp
                );
            } else {
                println!(
                    "  Preset {idx}: {}% brightness  hue={hue} sat={sat}",
                    brightness
                );
            }
        }
    }
}

// ── Plug display ─────────────────────────────────────────────────────────────

pub fn print_plug_summary(ip: IpAddr, plug: &Plug) {
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
        signal_label(plug.rssi),
        energy_tag,
    );
    println!("   {} HW:{}  FW:{}", plug.model, plug.hw_ver, short_fw(&plug.sw_ver));
    if plug.is_on() {
        println!("   On for: {}", plug.on_time_fmt());
    }
    let a = &plug.alias;
    let action = if plug.is_on() { "off" } else { "on" };
    let energy_hint = if plug.has_energy_monitoring() {
        format!("  ·  denki energy \"{a}\"")
    } else {
        String::new()
    };
    println!("   {}", format!("→ denki power \"{a}\" {action}{energy_hint}").dimmed());
    println!();
}

pub fn print_plug_detail(ip: &str, plug: &Plug) {
    println!("{}", header(&plug.alias));
    println!("  Host:     {ip}");
    println!("  State:    {}", on_state_detail(plug.is_on()));
    println!("  Model:    {}", plug.model);
    println!("  Hardware: {}", plug.hw_ver);
    println!("  Firmware: {}", plug.sw_ver);
    println!("  Signal:   {} dBm  {}", plug.rssi, signal_label(plug.rssi));
    println!("  LED:      {}", if plug.led_off == 1 { "off" } else { "on" });
    if plug.is_on() {
        println!("  On for:   {}", plug.on_time_fmt());
    }
    if plug.has_energy_monitoring() {
        println!("  {}", "Energy:     use `denki energy <host>`".dimmed());
        println!("  {}", "Daily:      use `denki energy-daily <host>`".dimmed());
        println!("  {}", "Monthly:    use `denki energy-monthly <host>`".dimmed());
        println!("  {}", "Schedule:   use `denki schedules <host>`".dimmed());
    }
}

pub fn print_schedules(json: &serde_json::Value) {
    if let Some(rules) = json
        .pointer("/schedule/get_rules/rule_list")
        .and_then(|v| v.as_array())
    {
        if rules.is_empty() {
            println!("No schedules configured.");
        } else {
            println!("{}", "Schedules:".bold());
            for r in rules {
                println!("  {:?}", r);
            }
        }
    }
}

// ── Energy display ────────────────────────────────────────────────────────────

pub fn print_energy_realtime(json: &serde_json::Value) {
    // Three possible response paths depending on device generation:
    //   KP115 (newer plug):  /emeter/get_realtime           — milli-units (mv, ma, mw, wh)
    //   HS110 (older plug):  /emeter/get_realtime           — real units (V, A, W, kWh)
    //   KL135 (bulb):        /smartlife.iot.common.emeter/get_realtime — milli-units, no V/A
    let data = json
        .pointer("/emeter/get_realtime")
        .or_else(|| json.pointer("/smartlife.iot.common.emeter/get_realtime"));

    if let Some(d) = data {
        // Safety net: if the device responded but reported an error (e.g. err_code -1
        // from HS105 which has no energy chip), show a clear message instead of silence
        if d.get("err_code").and_then(|v| v.as_i64()).unwrap_or(0) != 0 {
            let msg = d
                .get("err_msg")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            println!("{}", format!("Energy not supported: {msg}").yellow());
            return;
        }
        // KP115 reports in milli-units: power_mw, voltage_mv, current_ma, total_wh
        if let Some(mw) = d.get("power_mw").and_then(|v| v.as_f64()) {
            println!("Power:   {:.2} W", mw / 1000.0);
        }
        if let Some(mv) = d.get("voltage_mv").and_then(|v| v.as_f64()) {
            println!("Voltage: {:.1} V", mv / 1000.0);
        }
        if let Some(ma) = d.get("current_ma").and_then(|v| v.as_f64()) {
            println!("Current: {:.3} A", ma / 1000.0);
        }
        if let Some(wh) = d.get("total_wh").and_then(|v| v.as_u64()) {
            println!("Total:   {} Wh", wh);
        }

        // HS110 (older firmware) reports in real units: power, voltage, current, total (kWh)
        if let Some(w) = d.get("power").and_then(|v| v.as_f64()) {
            println!("Power:   {:.2} W", w);
        }
        if let Some(v) = d.get("voltage").and_then(|v| v.as_f64()) {
            println!("Voltage: {:.1} V", v);
        }
        if let Some(a) = d.get("current").and_then(|v| v.as_f64()) {
            println!("Current: {:.3} A", a);
        }
        if let Some(kwh) = d.get("total").and_then(|v| v.as_f64()) {
            println!("Total:   {:.3} kWh", kwh);
        }

        // KL135 only has power_mw and total_wh — voltage/current not available
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
        let mut sorted: Vec<_> = list.iter().collect();
        sorted.sort_by_key(|d| d["day"].as_u64().unwrap_or(0));
        for d in sorted {
            let day = d["day"].as_u64().unwrap_or(0);
            let wh = wh_from(d);
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
        for m in list {
            let month = m["month"].as_u64().unwrap_or(0);
            let wh = wh_from(m);
            let bar = "#".repeat((wh / 100).min(40) as usize);
            println!("  Month {:2}: {:5} Wh  {}", month, wh, bar.yellow());
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn signal_label(rssi: i32) -> colored::ColoredString {
    if rssi >= -50 {
        "excellent".green()
    } else if rssi >= -65 {
        "good".yellow()
    } else {
        "weak".red()
    }
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

// ── Light strip display ───────────────────────────────────────────────────────
// Light strips share the Bulb struct (same sysinfo shape) but use the
// smartlife.iot.lightStrip namespace instead of smartbulb.lightingservice.

pub fn print_lightstrip_summary(ip: IpAddr, bulb: &Bulb) {
    println!(
        "{} {} {} {} {}",
        header(&bulb.alias),
        "[light strip]".dimmed(),
        format!("[{ip}]").dimmed(),
        on_state(bulb.light_state.is_on()),
        signal_label(bulb.rssi),
    );
    println!("   {} HW:{}  FW:{}", bulb.model, bulb.hw_ver, short_fw(&bulb.sw_ver));
    print_light_color(&bulb.light_state, "   ");
    println!("   {}", "→ power/color control not yet implemented for KL430".dimmed());
    println!();
}

pub fn print_lightstrip_detail(ip: &str, bulb: &Bulb) {
    println!("{} {}", header(&bulb.alias), "[light strip]".dimmed());
    println!("  Host:       {ip}");
    println!("  State:      {}", on_state_detail(bulb.light_state.is_on()));
    println!("  Model:      {}", bulb.model);
    println!("  Hardware:   {}", bulb.hw_ver);
    println!("  Firmware:   {}", bulb.sw_ver);
    println!("  Signal:     {} dBm  {}", bulb.rssi, signal_label(bulb.rssi));
    let ls = &bulb.light_state;
    println!("  Brightness: {}%", ls.brightness());
    if ls.color_temp() > 0 {
        println!("  Warmth:     {}K", ls.color_temp());
    } else {
        println!("  Color:      {}° hue  {} sat", ls.hue(), ls.saturation());
    }
    println!("  {}", "NOTE: unverified — not tested on live hardware".yellow());
}

// ── Dimmer display ────────────────────────────────────────────────────────────

pub fn print_dimmer_summary(ip: IpAddr, d: &Dimmer) {
    println!(
        "{} {} {} {} {}",
        header(&d.alias),
        "[dimmer]".dimmed(),
        format!("[{ip}]").dimmed(),
        on_state(d.is_on()),
        signal_label(d.rssi),
    );
    println!("   {} HW:{}  FW:{}  {}%", d.model, d.hw_ver, short_fw(&d.sw_ver), d.brightness);
    let a = &d.alias;
    let action = if d.is_on() { "off" } else { "on" };
    println!(
        "   {}",
        format!("→ denki power \"{a}\" {action}  ·  denki dim \"{a}\" 80").dimmed()
    );
    println!();
}

pub fn print_dimmer_detail(ip: &str, d: &Dimmer) {
    println!("{} {}", header(&d.alias), "[dimmer]".dimmed());
    println!("  Host:       {ip}");
    println!("  State:      {}", on_state_detail(d.is_on()));
    println!("  Model:      {}", d.model);
    println!("  Hardware:   {}", d.hw_ver);
    println!("  Firmware:   {}", d.sw_ver);
    println!("  Signal:     {} dBm  {}", d.rssi, signal_label(d.rssi));
    println!("  Brightness: {}%", d.brightness);
    println!("  {}", "NOTE: unverified — not tested on live hardware".yellow());
}

// ── Strip display ─────────────────────────────────────────────────────────────

pub fn print_strip_summary(ip: IpAddr, s: &Strip) {
    let on_count = s.children.iter().filter(|c| c.is_on()).count();
    let total = s.children.len();
    let state = if on_count > 0 {
        format!("{on_count}/{total} on").green().bold()
    } else {
        "all off".dimmed().to_string().normal()
    };
    println!(
        "{} {} {} {}",
        header(&s.alias),
        "[strip]".dimmed(),
        format!("[{ip}]").dimmed(),
        state,
    );
    println!("   {} HW:{}  FW:{}", s.model, s.hw_ver, short_fw(&s.sw_ver));
    let a = &s.alias;
    println!(
        "   {}",
        format!("→ denki outlets \"{a}\"  ·  denki outlet \"{a}\" 1 on|off").dimmed()
    );
    println!();
}

pub fn print_strip_detail(ip: &str, s: &Strip) {
    let on_count = s.children.iter().filter(|c| c.is_on()).count();
    println!("{} {}", header(&s.alias), "[strip]".dimmed());
    println!("  Host:     {ip}");
    println!("  Model:    {}", s.model);
    println!("  Hardware: {}", s.hw_ver);
    println!("  Firmware: {}", s.sw_ver);
    println!("  Signal:   {} dBm  {}", s.rssi, signal_label(s.rssi));
    println!("  Outlets:  {}/{} on", on_count, s.children.len());
    print_strip_outlets(s);
    println!("  {}", "NOTE: unverified — not tested on live hardware".yellow());
}

pub fn print_strip_outlets(s: &Strip) {
    for (i, child) in s.children.iter().enumerate() {
        println!("  Outlet {}: {}  {}", i + 1, on_state(child.is_on()), child.alias);
    }
}

// ── Tapo device display ───────────────────────────────────────────────────────

pub fn print_tapo_summary(ip: IpAddr, d: &TapoDevice) {
    println!(
        "{} {} {} {}",
        header(&d.nickname),
        format!("[{ip}]").dimmed(),
        on_state(d.is_on()),
        tapo_signal_label(d.signal_level),
    );
    println!("   {} HW:{}  FW:{}", d.model, d.hw_ver, short_fw(&d.fw_ver));
    let a = &d.nickname;
    let action = if d.is_on() { "off" } else { "on" };
    println!("   {}", format!("→ denki power \"{a}\" {action}").dimmed());
    println!();
}

pub fn print_tapo_detail(ip: &str, d: &TapoDevice) {
    println!("{}", header(&d.nickname));
    println!("  Host:      {ip}");
    println!("  State:     {}", on_state_detail(d.is_on()));
    println!("  Model:     {}", d.model);
    println!("  Hardware:  {}", d.hw_ver);
    println!("  Firmware:  {}", d.fw_ver);
    println!("  Signal:    {} dBm  {}", d.rssi, tapo_signal_label(d.signal_level));
    if d.is_on() && d.on_time > 0 {
        println!("  On for:    {}", crate::fmt::duration(d.on_time));
    }
    if d.overheated {
        println!("  {}", "WARNING: device overheated".red().bold());
    }
}

fn tapo_signal_label(level: u8) -> colored::ColoredString {
    match level {
        3 => "excellent".green(),
        2 => "good".yellow(),
        1 => "weak".red(),
        _ => "no signal".red(),
    }
}

