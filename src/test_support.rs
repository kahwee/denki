use serde_json::Value;

pub fn hs300_hw2_sysinfo() -> Value {
    serde_json::from_str(include_str!("../tests/fixtures/hs300_hw2_sysinfo.json"))
        .expect("hs300_hw2_sysinfo fixture must stay valid JSON")
}

pub fn strip_child(id: &str, state: u8, alias: &str, on_time: u64) -> Value {
    serde_json::json!({
        "id": id,
        "state": state,
        "alias": alias,
        "on_time": on_time,
    })
}

pub fn strip_sysinfo(alias: &str, model: &str, feature: &str, children: Vec<Value>) -> Value {
    serde_json::json!({
        "system": { "get_sysinfo": {
            "alias": alias,
            "model": model,
            "hw_ver": "1.0",
            "sw_ver": "1.0.0",
            "rssi": -40,
            "feature": feature,
            "children": children
        }}
    })
}

pub fn clock_response(year: u64, month: u64, mday: u64, hour: u64, min: u64, sec: u64) -> Value {
    serde_json::json!({
        "time": { "get_time": { "year": year, "month": month, "mday": mday, "hour": hour, "min": min, "sec": sec } }
    })
}
