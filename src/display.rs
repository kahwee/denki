use crate::bulb::{Bulb, LightState};
use crate::devices;
use crate::dimmer::Dimmer;
use crate::plug::Plug;
use crate::strip::Strip;
use crate::tapo::TapoDevice;
use colored::{ColoredString, Colorize};
use std::net::IpAddr;

// ── Private helpers ───────────────────────────────────────────────────────────

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
                .map(|kwh| (kwh * 1000.0).round() as u64)
        })
        .unwrap_or(0)
}

/// Convert HSV (h: 0–360, s: 0–100, v: 0–100) to (r, g, b) each 0–255.
fn hsv_to_rgb(h: u16, s: u8, v: u8) -> (u8, u8, u8) {
    let s = s as f32 / 100.0;
    let v = v as f32 / 100.0;
    let h = h as f32;
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

/// Print the color temperature or RGB color line for a light state.
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

// ── Bulb display ─────────────────────────────────────────────────────────────

pub fn print_bulb_summary(ip: IpAddr, bulb: &Bulb) {
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
    let a = &bulb.alias;
    let action = if bulb.light_state.is_on() {
        "off"
    } else {
        "on"
    };
    let color_hint = if bulb.is_color == 1 {
        format!("  ·  denki color-temp \"{a}\" 2700  ·  denki dim \"{a}\" 80")
    } else if bulb.is_dimmable == 1 {
        format!("  ·  denki dim \"{a}\" 80")
    } else {
        String::new()
    };
    println!(
        "   {}",
        format!("→ denki {action} \"{a}\"{color_hint}").dimmed()
    );
    println!();
}

pub fn print_bulb_detail(ip: &str, bulb: &Bulb) {
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
    let ls = &bulb.light_state;
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
    println!("  Features:   {}", caps_label(bulb));
    let a = &bulb.alias;
    let is_on = bulb.light_state.is_on();
    let hints = devices::lookup(&bulb.model)
        .map(|e| devices::hints(e, a, is_on))
        .unwrap_or_else(|| {
            let action = if is_on { "off" } else { "on" };
            vec![format!("denki {action} \"{a}\"")]
        });
    println!("  {}", format!("→ {}", hints.join("  ·  ")).dimmed());
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
    let a = &plug.alias;
    let action = if plug.is_on() { "off" } else { "on" };
    let energy_hint = if plug.has_energy_monitoring() {
        format!("  ·  denki energy \"{a}\"")
    } else {
        String::new()
    };
    println!(
        "   {}",
        format!("→ denki {action} \"{a}\"{energy_hint}").dimmed()
    );
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
    println!(
        "  LED:      {}",
        if plug.led_off == 1 { "off" } else { "on" }
    );
    if plug.is_on() {
        println!("  On for:   {}", plug.on_time_fmt());
    }
    let a = &plug.alias;
    let is_on = plug.is_on();
    // Use devices.toml for model-accurate hints (HS105 omits energy; KP115 includes it).
    // Fall back to a minimal hint if the model isn't in the registry yet.
    let hints = devices::lookup(&plug.model)
        .map(|e| {
            // Runtime ENE flag overrides the static devices.toml entry:
            // if the sysinfo says no energy chip, drop the energy hint.
            let mut h = devices::hints(e, a, is_on);
            if !plug.has_energy_monitoring() {
                h.retain(|s| !s.contains("energy"));
            }
            h
        })
        .unwrap_or_else(|| {
            let action = if is_on { "off" } else { "on" };
            vec![format!("denki {action} \"{a}\"")]
        });
    println!("  {}", format!("→ {}", hints.join("  ·  ")).dimmed());
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
            let enabled = r.get("enable").and_then(|v| v.as_u64()).unwrap_or(0) == 1;
            let name = r
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("(unnamed)");
            let smin = r.get("smin").and_then(|v| v.as_u64()).unwrap_or(0);
            let sact = r.get("sact").and_then(|v| v.as_i64()).unwrap_or(1);
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
        if d.get("power_mw").is_some() {
            // KP115 / KL135 — milli-unit fields: power_mw, voltage_mv, current_ma, total_wh
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
        } else {
            // HS110 (older firmware) — real-unit fields: power (W), voltage (V), current (A), total (kWh)
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

/// For scan summary lines: "signal:excellent" with the quality word colored.
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
        signal_summary(bulb.rssi),
    );
    println!(
        "   {} HW:{}  FW:{}",
        bulb.model,
        bulb.hw_ver,
        short_fw(&bulb.sw_ver)
    );
    print_light_color(&bulb.light_state, "   ");
    let a = &bulb.alias;
    println!("   {}", format!("→ denki energy \"{a}\"  ·  denki energy-daily \"{a}\"  ·  denki energy-monthly \"{a}\"  ·  power/color control not yet implemented for KL430").dimmed());
    println!();
}

pub fn print_lightstrip_detail(ip: &str, bulb: &Bulb) {
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
    let ls = &bulb.light_state;
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
    println!(
        "  {}",
        "→ power and color control not yet implemented for KL430".dimmed()
    );
    println!(
        "  {}",
        "NOTE: unverified — not tested on live hardware".yellow()
    );
}

// ── Dimmer display ────────────────────────────────────────────────────────────

pub fn print_dimmer_summary(ip: IpAddr, d: &Dimmer) {
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
    let a = &d.alias;
    let action = if d.is_on() { "off" } else { "on" };
    println!(
        "   {}",
        format!("→ denki {action} \"{a}\"  ·  denki dim \"{a}\" 80").dimmed()
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
    let a = &d.alias;
    let is_on = d.is_on();
    let hints = devices::lookup(&d.model)
        .map(|e| devices::hints(e, a, is_on))
        .unwrap_or_else(|| {
            let action = if is_on { "off" } else { "on" };
            vec![format!("denki {action} \"{a}\"")]
        });
    println!("  {}", format!("→ {}", hints.join("  ·  ")).dimmed());
    println!(
        "  {}",
        "NOTE: unverified — not tested on live hardware".yellow()
    );
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
    // Outlet names colored by state: on = green bold, off = dimmed
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
    let a = &s.alias;
    let energy_hint = if s.has_energy_monitoring() {
        format!("  ·  denki energy \"{a}\" 1")
    } else {
        String::new()
    };
    println!(
        "   {}",
        format!(
            "→ denki outlets \"{a}\"  ·  denki on \"{a}\" 1  ·  denki off \"{a}\" 1{energy_hint}"
        )
        .dimmed()
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
    let a = &s.alias;
    // Start with model-derived hints from devices.toml, then append
    // outlet-specific commands that aren't in the generic feature list.
    let mut hints = devices::lookup(&s.model)
        .map(|e| {
            let mut h = devices::hints(e, a, s.children.iter().any(|c| c.is_on()));
            if !s.has_energy_monitoring() {
                h.retain(|s| !s.contains("energy"));
            }
            h
        })
        .unwrap_or_default();
    hints.push(format!("denki on \"{a}\" 1"));
    hints.push(format!("denki off \"{a}\" 1"));
    hints.push(format!("denki outlet-rename \"{a}\" 1 \"Name\""));
    if s.has_energy_monitoring() {
        hints.push(format!("denki energy \"{a}\" 1"));
    }
    println!("  {}", format!("→ {}", hints.join("  ·  ")).dimmed());
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

// ── Unknown device display ────────────────────────────────────────────────────

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
        format!("== {alias} ==").bold(),
        format!("[{ip}]").dimmed(),
        type_str.dimmed(),
    );
    println!("   {} {}", model, "— unsupported device type".dimmed());
    println!();
}

// ── Tapo device display ───────────────────────────────────────────────────────

pub fn print_tapo_summary(ip: IpAddr, d: &TapoDevice) {
    println!(
        "{} {} {} {}",
        header(&d.nickname),
        format!("[{ip}]").dimmed(),
        on_state(d.is_on()),
        format!("signal:{}", tapo_signal_label(d.signal_level)).as_str(),
    );
    println!("   {} HW:{}  FW:{}", d.model, d.hw_ver, short_fw(&d.fw_ver));
    let a = &d.nickname;
    let action = if d.is_on() { "off" } else { "on" };
    println!("   {}", format!("→ denki {action} \"{a}\"").dimmed());
    println!();
}

pub fn print_tapo_detail(ip: &str, d: &TapoDevice) {
    println!("{}", header(&d.nickname));
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
    let a = &d.nickname;
    let action = if d.is_on() { "off" } else { "on" };
    println!("  {}", format!("→ denki {action} \"{a}\"").dimmed());
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
mod tests {
    use super::*;
    use rstest::rstest;

    // ── short_fw ──────────────────────────────────────────────────────────────

    #[rstest]
    #[case("1.1.1 Build 250908 Rel.112945", "1.1.1")]
    #[case("1.0.15 Build 240429 Rel.154143", "1.0.15")]
    #[case("1.0.9 Build 250627 Rel.180045", "1.0.9")]
    #[case("1.0.9", "1.0.9")]
    #[case("", "")]
    fn short_fw_strips_build_suffix(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(short_fw(input), expected);
    }

    // ── hsv_to_rgb ────────────────────────────────────────────────────────────

    #[rstest]
    #[case(  0, 100, 100, (255,   0,   0))] // red
    #[case( 60, 100, 100, (255, 255,   0))] // yellow
    #[case(120, 100, 100, (  0, 255,   0))] // green
    #[case(180, 100, 100, (  0, 255, 255))] // cyan
    #[case(240, 100, 100, (  0,   0, 255))] // blue
    #[case(300, 100, 100, (255,   0, 255))] // magenta
    #[case(  0,   0, 100, (255, 255, 255))] // white (sat=0)
    #[case(  0,   0,   0, (  0,   0,   0))] // black (val=0)
    fn hsv_to_rgb_primary_colors(
        #[case] h: u16,
        #[case] s: u8,
        #[case] v: u8,
        #[case] expected: (u8, u8, u8),
    ) {
        assert_eq!(hsv_to_rgb(h, s, v), expected, "hsv({h},{s},{v})");
    }

    // ── wh_from ───────────────────────────────────────────────────────────────

    #[test]
    fn wh_from_integer_energy_wh() {
        let entry = serde_json::json!({"energy_wh": 1500});
        assert_eq!(wh_from(&entry), 1500);
    }

    #[test]
    fn wh_from_rounds_kwh_not_truncates() {
        // 1.9999 kWh truncated → 1999 Wh; rounded → 2000 Wh
        let entry = serde_json::json!({"energy": 1.9999});
        assert_eq!(wh_from(&entry), 2000);
    }

    #[test]
    fn wh_from_prefers_energy_wh_over_energy() {
        let entry = serde_json::json!({"energy_wh": 500, "energy": 1.0});
        assert_eq!(wh_from(&entry), 500);
    }

    #[test]
    fn hsv_to_rgb_kl135_purple_hue_308() {
        // hue=308 sat=65 val=100 is the default color on scanned bulbs
        // Purple range: high red, low green, moderate-high blue
        let (r, g, b) = hsv_to_rgb(308, 65, 100);
        assert!(r > g, "expected red > green for purple: ({r},{g},{b})");
        assert!(b > g, "expected blue > green for purple: ({r},{g},{b})");
    }
}
