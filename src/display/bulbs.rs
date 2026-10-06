use super::common::{
    header, on_state, on_state_detail, print_light_color, print_light_effect_detail,
    print_light_state_detail, short_fw, signal_label, signal_summary,
};
use super::hints::{bulb_hints, caps_label};
use crate::bulb::Bulb;
use colored::Colorize;
use std::net::IpAddr;

pub fn print_bulb_summary(ip: IpAddr, bulb: &Bulb, hint_alias: &str) {
    crate::output::println!(
        "{} {} {} {}",
        header(&bulb.alias),
        format!("[{ip}]").dimmed(),
        on_state(bulb.light_state.is_on()),
        signal_summary(bulb.rssi),
    );
    crate::output::println!(
        "   {} {}  HW:{}  FW:{}",
        bulb.model,
        caps_label(bulb).dimmed(),
        bulb.hw_ver,
        short_fw(&bulb.sw_ver),
    );
    print_light_color(&bulb.light_state, "   ");
    crate::output::println!(
        "   {}",
        format!("→ {}", bulb_hints(bulb, hint_alias).join("  ·  ")).dimmed()
    );
    crate::output::println!();
}

pub fn print_bulb_detail(ip: &str, bulb: &Bulb, hint_alias: &str) {
    crate::output::println!("{}", header(&bulb.alias));
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
    crate::output::println!("  Features:   {}", caps_label(bulb));
    crate::output::println!(
        "  {}",
        format!("→ {}", bulb_hints(bulb, hint_alias).join("  ·  ")).dimmed()
    );
}

pub fn print_bulb_specs(json: &serde_json::Value) {
    if let Some(s) = json.pointer("/smartlife.iot.smartbulb.lightingservice/get_light_details") {
        crate::output::println!(
            "Beam angle:              {}°",
            s["lamp_beam_angle"].as_u64().unwrap_or(0)
        );
        crate::output::println!(
            "Wattage:                 {}W",
            s["wattage"].as_u64().unwrap_or(0)
        );
        crate::output::println!(
            "Incandescent equivalent: {}W",
            s["incandescent_equivalent"].as_u64().unwrap_or(0)
        );
        crate::output::println!(
            "Max lumens:              {}",
            s["max_lumens"].as_u64().unwrap_or(0)
        );
        crate::output::println!(
            "Color rendering index:   {}",
            s["color_rendering_index"].as_u64().unwrap_or(0)
        );
        crate::output::println!(
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
        crate::output::println!("{}", "Saved presets:".bold());
        for s in states {
            let idx = s["index"].as_u64().unwrap_or(0) + 1;
            let brightness = s["brightness"].as_u64().unwrap_or(0);
            let color_temp = s["color_temp"].as_u64().unwrap_or(0);
            let hue = s["hue"].as_u64().unwrap_or(0);
            let sat = s["saturation"].as_u64().unwrap_or(0);
            if color_temp > 0 {
                crate::output::println!("  Preset {idx}: {brightness}% brightness  {color_temp}K");
            } else {
                crate::output::println!(
                    "  Preset {idx}: {brightness}% brightness  hue={hue} sat={sat}"
                );
            }
        }
    }
}
