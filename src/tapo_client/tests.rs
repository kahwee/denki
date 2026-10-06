use super::*;
use ::tapo::TapoResponseError;

fn fixture(on: bool) -> DeviceInfoPlugResult {
    serde_json::from_value(serde_json::json!({
        "avatar": "plug", "device_id": "fixture-id", "fw_id": "fixture-fw",
        "fw_ver": "0.0.0", "has_set_location_info": false, "hw_id": "fixture-hw",
        "hw_ver": "1.0", "ip": "192.0.2.1", "lang": "en_US",
        "mac": "00:00:00:00:00:00", "model": "P125", "oem_id": "fixture-oem",
        "rssi": -45, "signal_level": 3, "specs": "US", "ssid": "fixture-network",
        "type": "SMART.TAPOPLUG", "default_states": {"type": "last_states"},
        "device_on": on, "nickname": "TWFu", "on_time": 0
    }))
    .unwrap()
}

struct FakePlug {
    reads: std::sync::Mutex<std::collections::VecDeque<DeviceInfoPlugResult>>,
    writes: std::sync::Mutex<Vec<bool>>,
    fail_write: bool,
}

impl FakePlug {
    fn new(reads: Vec<DeviceInfoPlugResult>) -> Self {
        Self {
            reads: std::sync::Mutex::new(reads.into()),
            writes: Default::default(),
            fail_write: false,
        }
    }
}

impl PlugIo for FakePlug {
    async fn read_info(&self) -> std::result::Result<DeviceInfoPlugResult, Error> {
        Ok(self
            .reads
            .lock()
            .unwrap()
            .pop_front()
            .expect("unexpected extra read"))
    }
    async fn write_power(&self, on: bool) -> std::result::Result<(), Error> {
        self.writes.lock().unwrap().push(on);
        if self.fail_write {
            Err(Error::Tapo(TapoResponseError::Unauthorized {
                kind: "TPAP_AUTH_ATTEMPTS_LIMIT",
                description: "private".into(),
            }))
        } else {
            Ok(())
        }
    }
}

#[test]
fn normalization_keeps_already_decoded_nickname() {
    let info = normalize(fixture(false));
    assert_eq!(info.nickname, "TWFu"); // Valid base64 must still remain literal.
    assert_eq!(info.rssi, -45);
    assert!(!info.device_on);
}

#[tokio::test]
async fn power_actions_read_back_and_toggle_current_state() {
    for (before, requested, expected) in [
        (false, Some(true), true),
        (true, Some(false), false),
        (false, None, true),
        (true, None, false),
    ] {
        let device = FakePlug::new(vec![fixture(before), fixture(expected)]);
        assert_eq!(apply_power(&device, requested).await.unwrap(), expected);
        assert_eq!(*device.writes.lock().unwrap(), vec![expected]);
        assert!(device.reads.lock().unwrap().is_empty());
    }
}

#[tokio::test]
async fn unsupported_model_never_receives_a_write() {
    let mut info = fixture(false);
    info.model = "P110".into();
    let device = FakePlug::new(vec![info]);
    assert!(apply_power(&device, Some(true)).await.is_err());
    assert!(device.writes.lock().unwrap().is_empty());
}

#[tokio::test]
async fn uncertain_state_or_identity_never_retries_a_write() {
    for changed_identity in [false, true] {
        let mut after = fixture(changed_identity);
        if changed_identity {
            after.device_id = "different-fixture".into();
        }
        let device = FakePlug::new(vec![fixture(false), after]);
        assert!(
            apply_power(&device, Some(true))
                .await
                .unwrap_err()
                .to_string()
                .contains("readback")
        );
        assert_eq!(*device.writes.lock().unwrap(), vec![true]);
    }
    let mut device = FakePlug::new(vec![fixture(false)]);
    device.fail_write = true;
    assert!(
        apply_power(&device, None)
            .await
            .unwrap_err()
            .to_string()
            .contains("TPAP_AUTH_ATTEMPTS_LIMIT")
    );
    assert_eq!(*device.writes.lock().unwrap(), vec![true]);
}

#[test]
fn authentication_errors_are_actionable_and_redacted() {
    for kind in ["TPAP_CREDENTIALS", "TPAP_AUTH_ATTEMPTS_LIMIT"] {
        let error = Error::Tapo(TapoResponseError::Unauthorized {
            kind,
            description: "private account and raw payload".into(),
        });
        let message = format!("{:#}", upstream_error("login", error));
        assert!(message.contains(kind));
        assert!(!message.contains("private"));
    }
}

#[test]
fn parse_errors_preserve_stage_without_response_contents() {
    let error = serde_json::from_str::<bool>("\"private data\"").unwrap_err();
    let message = upstream_error("information read", Error::Serde(error)).to_string();
    assert!(message.contains("information read"));
    assert!(message.contains("parse"));
    assert!(!message.contains("private"));
}
