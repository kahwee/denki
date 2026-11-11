use super::support::{
    make_dimmer_for_hints, make_lightstrip_for_hints, make_plug_for_hints, make_strip_for_hints,
};
use crate::display::*;
use serde_json::json;

const HOSTILE: &str = "device\x1b]52;c;YQ==\x07\x1b[2J\r\n\t\u{9b}2J\u{202e}\u{2066}";

#[test]
fn render_fixture() {
    let Ok(mode) = std::env::var("DENKI_TERMINAL_FIXTURE") else {
        return;
    };
    colored::control::set_override(mode == "color");
    let render = || {
        let ip = "192.0.2.1".parse().unwrap();
        let effect = serde_json::from_value::<crate::bulb::LightingEffectState>(
            json!({"name":HOSTILE,"enable":1}),
        )
        .unwrap();
        let mut bulb = make_lightstrip_for_hints("KL135", true);
        bulb.alias = HOSTILE.into();
        bulb.model = HOSTILE.into();
        bulb.hw_ver = HOSTILE.into();
        bulb.sw_ver = HOSTILE.into();
        bulb.lighting_effect_state = Some(effect.clone());
        print_bulb_summary(ip, &bulb, HOSTILE);
        print_bulb_detail(HOSTILE, &bulb, HOSTILE);
        print_lightstrip_summary(ip, &bulb, "");
        print_lightstrip_detail(HOSTILE, &bulb, HOSTILE);
        let mut dimmer = make_dimmer_for_hints();
        dimmer.alias = HOSTILE.into();
        dimmer.model = HOSTILE.into();
        dimmer.hw_ver = HOSTILE.into();
        dimmer.sw_ver = HOSTILE.into();
        print_dimmer_summary(ip, &dimmer, "");
        print_dimmer_detail(HOSTILE, &dimmer, HOSTILE);
        let mut plug = make_plug_for_hints("KP115", false, true);
        plug.alias = HOSTILE.into();
        plug.model = HOSTILE.into();
        plug.hw_ver = HOSTILE.into();
        plug.sw_ver = HOSTILE.into();
        print_plug_summary(ip, &plug, "");
        print_plug_detail(HOSTILE, &plug, HOSTILE);
        let mut strip = make_strip_for_hints("HS300", true);
        strip.alias = HOSTILE.into();
        strip.model = HOSTILE.into();
        strip.hw_ver = HOSTILE.into();
        strip.sw_ver = HOSTILE.into();
        strip.children.push(
            serde_json::from_value(
                json!({"id":"synthetic", "alias":HOSTILE,"state":1,"on_time":0}),
            )
            .unwrap(),
        );
        print_strip_summary(ip, &strip, "");
        print_strip_detail(HOSTILE, &strip, HOSTILE);
        let tapo = crate::tapo::TapoDevice {
            model: HOSTILE.into(),
            hw_ver: HOSTILE.into(),
            fw_ver: HOSTILE.into(),
            device_on: false,
            on_time: 0,
            rssi: -42,
            signal_level: 3,
            nickname: HOSTILE.into(),
            overheated: false,
            device_id: "synthetic".into(),
        };
        print_tapo_summary(HOSTILE, &tapo, HOSTILE);
        print_tapo_detail(HOSTILE, &tapo, HOSTILE);
        print_unknown_summary(
            ip,
            &json!({"system":{"get_sysinfo":{"alias":HOSTILE,"model":HOSTILE}}}),
            HOSTILE,
        );
        print_schedules(
            &json!({"schedule":{"get_rules":{"rule_list":[{"name":HOSTILE,"enable":1,"wday":[1,1,1,1,1,1,1]}]}}}),
        );
        print_energy_realtime(&json!({"emeter":{"get_realtime":{"err_code":1,"err_msg":HOSTILE}}}));
        print_energy_daily(&json!({"emeter":{"get_daystat":{"day_list":[]}}}), HOSTILE);
        crate::effects::print_catalog(&effect);
        crate::output::record(json!({"alias":HOSTILE}));
    };
    if mode == "json" {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let (_, data) = runtime.block_on(crate::output::collect(true, async {
            render();
        }));
        std::println!("RAW_JSON:{data}");
    } else {
        render();
    }
}

fn run_fixture(mode: &str) -> std::process::Output {
    std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "display::tests::terminal::render_fixture",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("DENKI_TERMINAL_FIXTURE", mode)
        .output()
        .unwrap()
}

fn assert_terminal_safe(text: &str, colored: bool) {
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\n' {
            continue;
        } // Only our layout may introduce new lines.
        if c == '\x1b' && colored {
            assert_eq!(chars.next(), Some('['));
            while chars
                .peek()
                .is_some_and(|c| c.is_ascii_digit() || *c == ';')
            {
                chars.next();
            }
            assert_eq!(chars.next(), Some('m')); // Only trusted SGR styling survives.
            continue;
        }
        assert!(
            crate::output::is_terminal_safe(&c.to_string()),
            "unsafe character {c:?}"
        );
    }
    assert!(
        !text.contains("→"),
        "unsafe or absent targets must not produce command hints"
    );
    assert!(text.contains("\\u{1b}]52;c;YQ==\\u{7}\\u{1b}[2J\\r\\n\\t\\u{9b}2J\\u{202e}\\u{2066}"));
}

#[test]
fn all_device_renderers_escape_controls_before_terminal_styling() {
    for mode in ["plain", "color"] {
        let result = run_fixture(mode);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let stdout = String::from_utf8(result.stdout).unwrap();
        assert_terminal_safe(&stdout, mode == "color");
        if mode == "color" {
            assert!(stdout.contains("\x1b["));
        }
    }
}

#[test]
fn json_preserves_values_while_human_stderr_remains_safe() {
    let result = run_fixture("json");
    assert!(result.status.success());
    assert_terminal_safe(&String::from_utf8(result.stderr).unwrap(), false);
    let stdout = String::from_utf8(result.stdout).unwrap();
    let data = stdout
        .lines()
        .find_map(|line| line.split_once("RAW_JSON:").map(|(_, value)| value))
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(data).unwrap()["alias"],
        HOSTILE
    );
}
