use crate::bulb::{LightState, LightingEffectState};
use colored::{ColoredString, Colorize};

pub(crate) fn short_fw(fw: &str) -> &str {
    fw.split_whitespace().next().unwrap_or(fw)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(crate) fn wh_from(entry: &serde_json::Value) -> u64 {
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

#[allow(
    clippy::many_single_char_names,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub(crate) fn hsv_to_rgb(h: u16, s: u8, v: u8) -> (u8, u8, u8) {
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

pub(crate) fn on_state(is_on: bool) -> ColoredString {
    if is_on {
        "on".green().bold()
    } else {
        "off".dimmed()
    }
}

pub(crate) fn on_state_detail(is_on: bool) -> ColoredString {
    if is_on {
        "ON".green().bold()
    } else {
        "OFF".red()
    }
}

pub(crate) fn header(name: &str) -> ColoredString {
    format!("== {name} ==").bold()
}

pub(crate) fn signal_label(rssi: i32) -> colored::ColoredString {
    if rssi >= -50 {
        "excellent".green()
    } else if rssi >= -65 {
        "good".yellow()
    } else {
        "weak".red()
    }
}

pub(crate) fn signal_summary(rssi: i32) -> String {
    format!("signal:{}", signal_label(rssi))
}

pub(crate) fn tapo_signal_label(level: u8) -> colored::ColoredString {
    match level {
        3 => "excellent".green(),
        2 => "good".yellow(),
        1 => "weak".red(),
        _ => "no signal".red(),
    }
}

pub(crate) fn print_light_color(ls: &LightState, indent: &str) {
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

pub(crate) fn print_light_state_detail(ls: &LightState) {
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

pub(crate) fn print_light_effect_detail(effect: Option<&LightingEffectState>) {
    if let Some(effect) = effect {
        let name = if effect.enable == 1 {
            effect.name.as_str()
        } else {
            "Off"
        };
        println!("  Effect:     {}", name);
    }
}

pub(crate) fn format_energy_lines(d: &serde_json::Value) -> Vec<String> {
    let mut lines = Vec::new();
    if d.get("power_mw").is_some() {
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

pub(crate) fn format_wday(wday: Option<&Vec<serde_json::Value>>) -> String {
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

pub(crate) fn sort_energy_entries(list: &[serde_json::Value], key: &str) -> Vec<(u64, u64)> {
    let mut entries: Vec<(u64, u64)> = list
        .iter()
        .map(|e| (e[key].as_u64().unwrap_or(0), wh_from(e)))
        .collect();
    entries.sort_by_key(|(k, _)| *k);
    entries
}
