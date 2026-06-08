use crate::bulb::Bulb;
use crate::devices;
use crate::dimmer::Dimmer;
use crate::plug::Plug;
use crate::strip::Strip;

pub(crate) fn caps_label(bulb: &Bulb) -> String {
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

pub(crate) fn model_hints(model: &str, alias: &str, is_on: bool) -> Vec<String> {
    devices::lookup(model).map_or_else(
        || {
            let action = if is_on { "off" } else { "on" };
            vec![format!("denki {action} \"{alias}\"")]
        },
        |e| devices::hints(e, alias, is_on),
    )
}

pub(crate) fn bulb_hints(bulb: &Bulb, alias: &str) -> Vec<String> {
    model_hints(&bulb.model, alias, bulb.light_state.is_on())
}

pub(crate) fn plug_hints(plug: &Plug, alias: &str) -> Vec<String> {
    let mut h = model_hints(&plug.model, alias, plug.is_on());
    if !plug.has_energy_monitoring() {
        h.retain(|s| !s.contains("energy"));
    }
    h
}

pub(crate) fn dimmer_hints(d: &Dimmer, alias: &str) -> Vec<String> {
    model_hints(&d.model, alias, d.is_on())
}

pub(crate) fn lightstrip_hints(bulb: &Bulb, alias: &str) -> Vec<String> {
    devices::lookup(&bulb.model)
        .map(|e| {
            let mut h = devices::hints(e, alias, bulb.light_state.is_on());
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

pub(crate) fn strip_hints(s: &Strip, alias: &str) -> Vec<String> {
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
