//! Real CLI subprocesses talking to a local Kasa TCP peer. No physical devices.
#![cfg(target_os = "linux")]
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::{Command, Output};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

struct Peer {
    stop: Arc<AtomicBool>,
    requests: Arc<Mutex<Vec<Value>>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Peer {
    fn start(reply: impl Fn(&Value, usize) -> Value + Send + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:9999").unwrap();
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let (stopped, received) = (stop.clone(), requests.clone());
        let thread = std::thread::spawn(move || {
            while !stopped.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut socket, _)) => {
                        socket
                            .set_read_timeout(Some(Duration::from_secs(3)))
                            .unwrap();
                        let mut size = [0; 4];
                        if socket.read_exact(&mut size).is_err() {
                            continue;
                        }
                        let mut body = vec![0; u32::from_be_bytes(size) as usize];
                        socket.read_exact(&mut body).unwrap();
                        let request: Value =
                            serde_json::from_slice(&denki::cipher::decode(&body)).unwrap();
                        let n = {
                            let mut log = received.lock().unwrap();
                            log.push(request.clone());
                            log.len()
                        };
                        let response = reply(&request, n);
                        let _ = socket.write_all(&denki::cipher::encode(
                            &serde_json::to_vec(&response).unwrap(),
                        ));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2))
                    }
                    Err(error) => panic!("{error}"),
                }
            }
        });
        Self {
            stop,
            requests,
            thread: Some(thread),
        }
    }
    fn mutations(&self) -> usize {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.pointer("/system/set_relay_state").is_some())
            .count()
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.thread.take().unwrap().join().unwrap();
    }
}
fn info() -> Value {
    json!({"system":{"get_sysinfo":{
        "err_code":0,"model":"KP115(US)","mic_type":"IOT.SMARTPLUGSWITCH",
        "alias":"Factory Name","deviceId":"synthetic-device-a","mac":"synthetic-mac",
        "hw_ver":"1.0","sw_ver":"1.0","rssi":-42,"relay_state":0,"feature":"TIM:ENE","on_time":0
    }}})
}
fn reply(request: &Value) -> Value {
    if request.pointer("/system/get_sysinfo").is_some() {
        return info();
    }
    if request.pointer("/emeter/get_realtime").is_some() {
        return json!({"emeter":{"get_realtime":{"err_code":0,"power_mw":1234,"voltage_mv":230000,"current_ma":20,"total_wh":56}}});
    }
    if request.pointer("/emeter/get_daystat").is_some() {
        return json!({"emeter":{"get_daystat":{"err_code":0,"day_list":[{"day":2,"energy":0.012},{"day":1,"energy_wh":5}]}}});
    }
    json!({"system":{"set_relay_state":{"err_code":0}}})
}
fn config() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("denki")).unwrap();
    dir
}
fn aliases(config: &tempfile::TempDir, value: Value) {
    std::fs::write(config.path().join("denki/hosts.json"), value.to_string()).unwrap();
}
fn command(config: &tempfile::TempDir, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_denki"));
    command
        .env("XDG_CONFIG_HOME", config.path())
        .env_remove("TAPO_USER")
        .env_remove("TAPO_PASS")
        .args(args);
    command
}
fn run(config: &tempfile::TempDir, args: &[&str]) -> Output {
    command(config, args).output().unwrap()
}
fn document(output: &Output) -> Value {
    let value: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "Invalid JSON: {e}\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    });
    assert_eq!(value["schema_version"], 2);
    assert_eq!(output.status.success(), value["status"] == "ok", "{value}");
    value
}

#[test]
fn cli_real_tcp_success_failure_identity_groups_and_streams() {
    let config = config();
    aliases(
        &config,
        json!({"desk":{"ip":"127.0.0.1","protocol":"kasa","device_id":"synthetic-device-a"}}),
    );
    {
        let peer = Peer::start(|request, _| reply(request));
        let value = document(&run(&config, &["info", "desk", "--json"]));
        assert_eq!(value["data"]["device"]["model"], "KP115(US)");
        assert!(!value.to_string().contains("synthetic-device-a"));
        assert!(!value.to_string().contains("synthetic-mac"));
        let value = document(&run(&config, &["energy", "desk", "--json"]));
        assert_eq!(value["data"]["measurement"]["power_w"], 1.234);
        assert_eq!(value["data"]["measurement"]["energy_wh"], 56.0);
        let text = run(&config, &["energy", "desk"]);
        assert!(text.status.success());
        assert!(String::from_utf8_lossy(&text.stdout).contains("1.234 W"));
        let history = document(&run(
            &config,
            &["energy-daily", "desk", "2026-10", "--json"],
        ));
        assert_eq!(
            history["data"]["entries"][0],
            json!({"day":1,"energy_wh":5.0})
        );
        assert_eq!(history["data"]["entries"][1]["energy_wh"], 12.0);
        assert_eq!(
            document(&run(&config, &["toggle", "desk", "--json"]))["data"]["power_on"],
            true
        );
        assert_eq!(peer.mutations(), 1);
        let watched = run(
            &config,
            &[
                "energy",
                "watch",
                "desk",
                "--format",
                "jsonl",
                "--count",
                "2",
                "--interval",
                "1",
            ],
        );
        assert!(
            watched.status.success(),
            "{}",
            String::from_utf8_lossy(&watched.stderr)
        );
        let rows: Vec<Value> = String::from_utf8(watched.stdout)
            .unwrap()
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect();
        assert_eq!(rows.len(), 2);
        assert!(
            rows[1]["timestamp_unix_ms"].as_u64().unwrap()
                >= rows[0]["timestamp_unix_ms"].as_u64().unwrap() + 1000
        );
        assert_eq!(rows[0]["measurement"]["power_w"], 1.234);
        let csv = run(
            &config,
            &["energy", "watch", "desk", "--format", "csv", "--count", "1"],
        );
        let csv = String::from_utf8(csv.stdout).unwrap();
        assert_eq!(csv.lines().count(), 2, "{csv}");
        assert!(csv.starts_with("timestamp_unix_ms,device,"));
        assert!(csv.contains(",desk,,ok,1.234,"));
        assert_eq!(peer.mutations(), 1, "energy reads must never mutate");
    }
    // Missing/wrong state must not cause a toggle, including group toggle.
    for state in [Value::Null, json!("0"), json!(2)] {
        let peer = Peer::start(move |_, _| {
            let mut value = info();
            value["system"]["get_sysinfo"]["relay_state"] = state.clone();
            value
        });
        for args in [
            vec!["toggle", "desk", "--json"],
            vec!["group", "toggle", "desk", "--json"],
            vec!["doctor", "desk", "--json"],
        ] {
            let output = run(&config, &args);
            let value = document(&output);
            assert!(!output.status.success());
            assert!(value.to_string().contains("malformed_response"), "{value}");
            if args[0] == "doctor" {
                assert_eq!(value["data"]["reachable"], true);
            }
        }
        assert_eq!(peer.mutations(), 0);
    }
    // Device rejection and invalid success envelopes are errors in BOTH output modes.
    for response in [
        json!({"emeter":{"get_realtime":{"err_code":-2001}}}),
        json!({"emeter":{"get_realtime":{"power_mw":1000}}}),
        json!({"emeter":{"get_realtime":{"err_code":0}}}),
    ] {
        let _peer = Peer::start(move |request, _| {
            if request.pointer("/system/get_sysinfo").is_some() {
                info()
            } else {
                response.clone()
            }
        });
        assert!(!run(&config, &["energy", "desk"]).status.success());
        assert_eq!(
            document(&run(&config, &["energy", "desk", "--json"]))["status"],
            "error"
        );
    }
    // Reassigned address must be rejected before any write.
    {
        let peer = Peer::start(|_, _| {
            let mut value = info();
            value["system"]["get_sysinfo"]["deviceId"] = json!("synthetic-device-b");
            value
        });
        let value = document(&run(&config, &["on", "desk", "--json"]));
        assert_eq!(value["error"]["code"], "identity_mismatch");
        assert_eq!(peer.mutations(), 0);
    }
    // Reconcile an actual address change then use the retained custom name via the CLI.
    {
        let _peer = Peer::start(|r, _| reply(r));
        let mut map = std::collections::BTreeMap::new();
        map.insert(
            "custom desk".into(),
            denki::hosts::HostEntry {
                ip: "192.0.2.9".into(),
                protocol: denki::hosts::Protocol::Kasa,
                device_id: Some("synthetic-device-a".into()),
            },
        );
        assert!(
            denki::hosts::reconcile_in(
                "Factory Name",
                "127.0.0.1",
                denki::hosts::Protocol::Kasa,
                Some("synthetic-device-a"),
                &mut map
            )
            .unwrap()
        );
        aliases(&config, serde_json::to_value(map).unwrap());
        assert_eq!(
            document(&run(&config, &["on", "custom desk", "--json"]))["status"],
            "ok"
        );
    }
    // Partial failure retains every result and returns nonzero.
    {
        let peer = Peer::start(|r, _| reply(r));
        aliases(
            &config,
            json!({"office one":{"ip":"127.0.0.1","protocol":"kasa"},"office two":{"ip":"127.0.0.2","protocol":"kasa"}}),
        );
        let dry = document(&run(
            &config,
            &["group", "off", "office", "--dry-run", "--json"],
        ));
        assert_eq!(dry["data"]["targets"].as_array().unwrap().len(), 2);
        assert!(peer.requests.lock().unwrap().is_empty());
        let value = document(&run(&config, &["group", "off", "office", "--json"]));
        assert_eq!(value["error"]["code"], "partial_failure");
        assert_eq!(value["data"]["succeeded"], 1);
        assert_eq!(value["data"]["failed"], 1);
        assert_eq!(value["data"]["results"].as_array().unwrap().len(), 2);
    }
    // Failed poll is a missing sample, never zero; reconnect and recover next time.
    {
        let reads = Arc::new(AtomicBool::new(false));
        let _peer = Peer::start(move |r, _| {
            if r.pointer("/emeter/get_realtime").is_some() && !reads.swap(true, Ordering::Relaxed) {
                json!({"emeter":{"get_realtime":{"err_code":-2001}}})
            } else {
                reply(r)
            }
        });
        let output = run(
            &config,
            &[
                "energy",
                "watch",
                "127.0.0.1",
                "--json",
                "--count",
                "2",
                "--interval",
                "1",
            ],
        );
        assert!(!output.status.success());
        let rows: Vec<Value> = String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["status"], "error");
        assert!(rows[0]["measurement"].is_null());
        assert_eq!(rows[1]["status"], "ok");
    }
    // Ctrl-C stops an unbounded watch cleanly after a completed sample.
    {
        let _peer = Peer::start(|r, _| reply(r));
        let mut child = command(
            &config,
            &[
                "energy",
                "watch",
                "127.0.0.1",
                "--format",
                "jsonl",
                "--interval",
                "60",
            ],
        )
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
        let mut reader = std::io::BufReader::new(child.stdout.take().unwrap());
        let mut first = String::new();
        std::io::BufRead::read_line(&mut reader, &mut first).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&first).unwrap()["status"],
            "ok"
        );
        assert!(
            Command::new("kill")
                .args(["-INT", &child.id().to_string()])
                .status()
                .unwrap()
                .success()
        );
        let started = std::time::Instant::now();
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            if started.elapsed() > Duration::from_secs(3) {
                child.kill().unwrap();
                panic!("watch ignored Ctrl-C");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    assert_eq!(
        document(&run(
            &config,
            &[
                "energy", "watch", "desk", "--json", "--format", "csv", "--count", "1"
            ]
        ))["error"]["code"],
        "invalid_arguments"
    );
    assert_eq!(
        document(&run(&config, &["info", "missing", "--json"]))["error"]["code"],
        "not_found"
    );
    assert_eq!(
        document(&run(&config, &["dim", "desk", "999", "--json"]))["error"]["code"],
        "invalid_arguments"
    );
    let aliases = document(&run(&config, &["aliases", "--json"]));
    assert_eq!(aliases["data"]["aliases"].as_array().unwrap().len(), 2);
    let completions = document(&run(&config, &["completions", "bash", "--json"]));
    assert!(
        completions["data"]["script"]
            .as_str()
            .unwrap()
            .contains("denki")
    );
}
