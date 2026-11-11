//! Real CLI subprocesses talking to a local Kasa TCP peer. No physical devices.
#![cfg(target_os = "linux")]
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::{Command, Output};
use std::sync::{
    Arc, Mutex, MutexGuard,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

static PEER_PORT: Mutex<()> = Mutex::new(());

struct Peer {
    _port: MutexGuard<'static, ()>,
    stop: Arc<AtomicBool>,
    requests: Arc<Mutex<Vec<Value>>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Peer {
    fn start(reply: impl Fn(&Value, usize) -> Value + Send + 'static) -> Self {
        let port = PEER_PORT
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
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
                        let length = u32::from_be_bytes(size) as usize;
                        assert!(length <= 1024 * 1024, "oversized request to simulator");
                        let mut body = vec![0; length];
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
            _port: port,
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
            .filter(|r| !is_read_request(r))
            .count()
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let result = self.thread.take().unwrap().join();
        if !std::thread::panicking() {
            result.expect("simulated device failed");
        }
    }
}
fn info() -> Value {
    json!({"system":{"get_sysinfo":{
        "err_code":0,"model":"KP115(US)","mic_type":"IOT.SMARTPLUGSWITCH",
        "alias":"Factory Name","deviceId":"synthetic-device-a","mac":"synthetic-mac",
        "hw_ver":"1.0","sw_ver":"1.0","rssi":-42,"relay_state":0,"feature":"TIM:ENE","on_time":0
    }}})
}
fn is_read_request(request: &Value) -> bool {
    [
        json!({"system":{"get_sysinfo":{}}}),
        json!({"emeter":{"get_realtime":{}}}),
    ]
    .contains(request)
        || request.as_object().is_some_and(|root| {
            root.len() == 1
                && root
                    .get("emeter")
                    .and_then(Value::as_object)
                    .is_some_and(|emeter| emeter.len() == 1 && emeter.contains_key("get_daystat"))
        })
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
    assert!(
        request.pointer("/system/set_relay_state").is_some(),
        "unexpected simulator command: {request}"
    );
    json!({"system":{"set_relay_state":{"err_code":0}}})
}
fn new_config() -> tempfile::TempDir {
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
fn normalized_output_and_read_only_streams() {
    let config = new_config();
    aliases(
        &config,
        json!({"desk":{"ip":"127.0.0.1","protocol":"kasa","group_approved":true,"device_id":"synthetic-device-a"}}),
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
                "1",
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
        assert_eq!(rows.len(), 1);
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
}

#[test]
fn diagnostics_reject_malformed_state_without_writes() {
    let config = new_config();
    aliases(
        &config,
        json!({"desk":{"ip":"127.0.0.1","protocol":"kasa","group_approved":true,"device_id":"synthetic-device-a"}}),
    );
    // Weak signal is actionable, but does not turn a successful read into a failure.
    {
        let peer = Peer::start(|_, _| {
            let mut value = info();
            value["system"]["get_sysinfo"]["rssi"] = json!(-78);
            value
        });
        let text = run(&config, &["doctor", "desk"]);
        assert!(text.status.success());
        let text = String::from_utf8(text.stdout).unwrap();
        assert!(text.contains("Wi-Fi RSSI:   -78 dBm"));
        assert!(text.contains("Next step:"));
        let value = document(&run(&config, &["doctor", "desk", "--json"]));
        assert_eq!(value["data"]["signal_dbm"], -78);
        assert!(
            value["data"]["next_steps"][0]
                .as_str()
                .unwrap()
                .contains("coverage")
        );
        assert_eq!(peer.mutations(), 0, "doctor must remain read-only");
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
}

#[test]
fn device_rejections_and_identity_rebinds() {
    let config = new_config();
    aliases(
        &config,
        json!({"desk":{"ip":"127.0.0.1","protocol":"kasa","group_approved":true,"device_id":"synthetic-device-a"}}),
    );
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
    // An unauthenticated copied ID cannot redirect a trusted alias to the peer.
    // Only an explicit alias update may accept a verified Kasa address change.
    {
        let peer = Peer::start(|r, _| reply(r));
        let mut map = std::collections::BTreeMap::new();
        map.insert(
            "custom desk".into(),
            denki::hosts::HostEntry {
                ip: "127.0.0.2".into(),
                protocol: denki::hosts::Protocol::Kasa,
                device_id: Some("synthetic-device-a".into()),
                group_approved: false,
                physically_confirmed: false,
            },
        );
        let original = map.clone();
        let error = denki::hosts::reconcile_in(
            "Factory Name",
            "127.0.0.1",
            denki::hosts::Protocol::Kasa,
            Some("synthetic-device-a"),
            &mut map,
        )
        .unwrap_err();
        assert_eq!(denki::error::code(&error), "identity_rebind_required");
        assert_eq!(map, original);
        aliases(&config, serde_json::to_value(map).unwrap());
        assert!(
            !run(&config, &["on", "custom desk", "--json"])
                .status
                .success()
        );
        assert!(peer.requests.lock().unwrap().is_empty());
        assert_eq!(peer.mutations(), 0);
        assert!(
            run(&config, &["alias", "custom desk", "127.0.0.1"])
                .status
                .success()
        );
        assert_eq!(
            document(&run(&config, &["on", "custom desk", "--json"]))["status"],
            "ok"
        );
    }
}

#[test]
fn group_dry_run_and_partial_failure() {
    let config = new_config();
    aliases(
        &config,
        json!({"desk":{"ip":"127.0.0.1","protocol":"kasa","group_approved":true,"device_id":"synthetic-device-a"}}),
    );
    // Partial failure retains every result and returns nonzero.
    {
        let peer = Peer::start(|r, _| reply(r));
        aliases(
            &config,
            json!({"office one":{"ip":"127.0.0.1","protocol":"kasa","group_approved":true},"office two":{"ip":"127.0.0.2","protocol":"kasa","group_approved":true}}),
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
}

#[test]
fn poll_recovers_without_inventing_measurements() {
    let config = new_config();
    aliases(
        &config,
        json!({"desk":{"ip":"127.0.0.1","protocol":"kasa","group_approved":true,"device_id":"synthetic-device-a"}}),
    );
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
        assert!(
            rows[1]["timestamp_unix_ms"].as_u64().unwrap()
                >= rows[0]["timestamp_unix_ms"].as_u64().unwrap() + 1000
        );
    }
}

#[test]
fn watch_interrupt_and_cli_contracts() {
    let config = new_config();
    aliases(
        &config,
        json!({"desk":{"ip":"127.0.0.1","protocol":"kasa","group_approved":true,"device_id":"synthetic-device-a"}}),
    );
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
    let alias_output = document(&run(&config, &["aliases", "--json"]));
    assert_eq!(alias_output["data"]["aliases"].as_array().unwrap().len(), 1);
}

#[test]
fn fleet_deduplication_identification_and_deadline() {
    let config = new_config();
    aliases(
        &config,
        json!({"desk":{"ip":"127.0.0.1","protocol":"kasa","group_approved":true,"device_id":"synthetic-device-a"}}),
    );
    // Fleet checks retain partial failures, deduplicate logins, and never write power.
    {
        let fleet_config = new_config();
        aliases(
            &fleet_config,
            json!({
                "first":{"ip":"127.0.0.1","protocol":"kasa"},
                "second":{"ip":"127.0.0.1","protocol":"kasa"},
                "offline":{"ip":"127.0.0.2","protocol":"kasa"}
            }),
        );
        let peer = Peer::start(|request, _| reply(request));
        let identified = document(&run(
            &fleet_config,
            &["identify", "first", "--confirm", "--json"],
        ));
        assert_eq!(identified["data"]["physically_confirmed"], true);
        assert!(peer.requests.lock().unwrap().is_empty());
        let value = document(&run(
            &fleet_config,
            &["doctor", "--all", "--timeout", "1", "--json"],
        ));
        assert_eq!(value["error"]["code"], "fleet_diagnostics_failed");
        assert_eq!(value["data"]["summary"]["total"], 2);
        assert_eq!(value["data"]["summary"]["ok"], 1);
        assert_eq!(
            value["data"]["devices"][0]["aliases"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(peer.requests.lock().unwrap().len(), 1);
        assert_eq!(peer.mutations(), 0);
        assert_eq!(
            document(&run(
                &fleet_config,
                &["identify", "fir", "--confirm", "--json"]
            ))["error"]["code"],
            "not_found"
        );
        assert!(
            run(&fleet_config, &["alias", "first", "127.0.0.1"])
                .status
                .success()
        );
        let saved: Value = serde_json::from_str(
            &std::fs::read_to_string(fleet_config.path().join("denki/hosts.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(saved["first"]["physically_confirmed"], true);
        assert!(
            run(&fleet_config, &["alias", "first", "127.0.0.3"])
                .status
                .success()
        );
        let saved: Value = serde_json::from_str(
            &std::fs::read_to_string(fleet_config.path().join("denki/hosts.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(saved["first"]["physically_confirmed"], false);
        assert!(
            run(&fleet_config, &["identify", "second", "--confirm"])
                .status
                .success()
        );
        assert!(
            run(&fleet_config, &["identify", "second", "--clear"])
                .status
                .success()
        );
        aliases(
            &fleet_config,
            json!({"a":{"ip":"127.0.0.1","protocol":"kasa"},"b":{"ip":"127.0.0.1","protocol":"tapo"}}),
        );
        let conflict = document(&run(&fleet_config, &["doctor", "--all", "--json"]));
        assert_eq!(
            conflict["data"]["devices"][0]["diagnostics"]["error_code"],
            "alias_conflict"
        );
        assert_eq!(peer.requests.lock().unwrap().len(), 1);
    }
    {
        let fleet_config = new_config();
        aliases(
            &fleet_config,
            json!({"slow":{"ip":"127.0.0.1","protocol":"kasa"}}),
        );
        let (release, blocked) = std::sync::mpsc::channel();
        let peer = Peer::start(move |request, _| {
            blocked
                .recv_timeout(Duration::from_secs(5))
                .expect("test must release stalled peer");
            reply(request)
        });
        let value = document(&run(
            &fleet_config,
            &["doctor", "--all", "--timeout", "1", "--json"],
        ));
        release.send(()).unwrap();
        assert_eq!(
            value["data"]["devices"][0]["diagnostics"]["error_code"],
            "timeout"
        );
        assert_eq!(peer.requests.lock().unwrap().len(), 1);
        assert_eq!(peer.mutations(), 0);
    }
    let completions = document(&run(&config, &["completions", "bash", "--json"]));
    assert!(
        completions["data"]["script"]
            .as_str()
            .unwrap()
            .contains("denki")
    );
}
