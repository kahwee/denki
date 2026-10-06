//! Independent local HTTP peer exercises the actual handshake, AES framing, and ops.
use super::*;
use serde_json::{Value, json};
use tokio::net::TcpListener;

async fn peer(replies: Vec<Value>) -> (u16, tokio::task::JoinHandle<Vec<Value>>) {
    peer_with_corruption(replies, false).await
}
async fn peer_with_corruption(
    replies: Vec<Value>,
    corrupt: bool,
) -> (u16, tokio::task::JoinHandle<Vec<Value>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let task = tokio::spawn(async move {
        let remote_seed = [0x35; 16];
        let auth = auth_hash("test@example.invalid", "test-password");
        let mut local_seed = Vec::new();
        let mut requests = Vec::new();
        // Two handshakes, followed by a sequence of actual encrypted operations.
        for step in 0..(2 + replies.len()) {
            let (mut socket, _) = listener.accept().await.unwrap();
            let headers = String::from_utf8(read_headers(&mut socket).await.unwrap()).unwrap();
            let length = response_content_length(&headers).unwrap();
            let mut body = vec![0; length];
            socket.read_exact(&mut body).await.unwrap();
            let mut response = match step {
                0 => {
                    assert!(headers.starts_with("POST /app/handshake1 "));
                    assert_eq!(body.len(), 16);
                    local_seed = body;
                    let proof = sha256_multi(&[&local_seed, &remote_seed, &auth]);
                    [remote_seed.as_slice(), &proof].concat()
                }
                1 => {
                    assert!(headers.starts_with("POST /app/handshake2 "));
                    assert_eq!(body, sha256_multi(&[&remote_seed, &local_seed, &auth]));
                    assert!(headers.contains("Cookie: TP_SESSIONID=test-session"));
                    Vec::new()
                }
                _ => {
                    let path = headers.split_whitespace().nth(1).unwrap();
                    let seq: i32 = path.split("seq=").nth(1).unwrap().parse().unwrap();
                    let key: [u8; 16] = sha256_multi(&[b"lsk", &local_seed, &remote_seed, &auth])
                        [..16]
                        .try_into()
                        .unwrap();
                    let full_iv = sha256_multi(&[b"iv", &local_seed, &remote_seed, &auth]);
                    assert_eq!(
                        seq,
                        i32::from_be_bytes(full_iv[28..32].try_into().unwrap())
                            .wrapping_add((step - 1) as i32)
                    );
                    let mut iv = [0u8; 16];
                    iv[..12].copy_from_slice(&full_iv[..12]);
                    iv[12..].copy_from_slice(&seq.to_be_bytes());
                    let signing = sha256_multi(&[b"ldk", &local_seed, &remote_seed, &auth]);
                    let tag = sha256_multi(&[&signing[..28], &seq.to_be_bytes(), &body[32..]]);
                    assert_eq!(&body[..32], tag.as_slice());
                    let plain = Aes128CbcDec::new(&key.into(), &iv.into())
                        .decrypt_padded_vec::<Pkcs7>(&body[32..])
                        .unwrap();
                    let request: Value = serde_json::from_slice(&plain).unwrap();
                    assert!(request["method"].is_string());
                    requests.push(request);
                    let ciphertext = Aes128CbcEnc::new(&key.into(), &iv.into())
                        .encrypt_padded_vec::<Pkcs7>(
                            &serde_json::to_vec(&replies[step - 2]).unwrap(),
                        );
                    let tag = sha256_multi(&[&signing[..28], &seq.to_be_bytes(), &ciphertext]);
                    [tag.as_slice(), &ciphertext].concat()
                }
            };
            if corrupt && step >= 2 {
                response[0] ^= 1;
            }
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nSet-Cookie: TP_SESSIONID=test-session; Path=/\r\n\r\n",
                response.len()
            );
            socket.write_all(header.as_bytes()).await.unwrap();
            socket.write_all(&response).await.unwrap();
        }
        // Watch for forbidden mutation requests after an invalid state response.
        if let Ok(Ok((mut socket, _))) =
            tokio::time::timeout(Duration::from_millis(150), listener.accept()).await
        {
            let headers = read_headers(&mut socket).await.unwrap();
            requests.push(json!({"unexpected_request":String::from_utf8_lossy(&headers)}));
        }
        requests
    });
    (port, task)
}
#[tokio::test]
async fn malformed_tapo_state_and_envelopes_never_send_mutations() {
    for response in [
        json!({}),
        json!({"error_code":"0"}),
        json!({"error_code":-1501}),
        json!({"error_code":0,"result":{"device_on":null}}),
        json!({"error_code":0,"result":{"device_on":"false"}}),
    ] {
        let (port, peer) = peer(vec![response]).await;
        let mut session = handshake_at("127.0.0.1", port, "test@example.invalid", "test-password")
            .await
            .unwrap();
        let error = crate::ops::tapo_toggle(&mut session).await.unwrap_err();
        assert!(matches!(
            crate::error::code(&error),
            "malformed_response" | "device_rejected"
        ));
        assert_eq!(
            peer.await.unwrap().len(),
            1,
            "must not send a mutation after invalid device info"
        );
    }
}
#[tokio::test]
async fn valid_tapo_device_info_round_trips_over_http_and_aes() {
    let response = json!({"error_code":0,"result":{"model":"P125","hw_ver":"1.0","fw_ver":"1.0","rssi":-40,"device_on":false,"device_id":"synthetic-tapo","nickname":"VGVzdA=="}});
    let (port, peer) = peer(vec![response.clone()]).await;
    let mut session = handshake_at("127.0.0.1", port, "test@example.invalid", "test-password")
        .await
        .unwrap();
    assert_eq!(
        crate::ops::tapo_device_info(&mut session).await.unwrap(),
        response
    );
    assert_eq!(peer.await.unwrap().len(), 1);
}

#[tokio::test]
async fn tapo_mutation_acknowledgments_are_strict_and_session_sequences_advance() {
    let info = json!({"error_code":0,"result":{"model":"P125","hw_ver":"1.0","fw_ver":"1.0","rssi":-40,"device_on":false,"device_id":"synthetic-tapo"}});
    for (ack, expected) in [
        (json!({}), Some("malformed_response")),
        (json!({"error_code":"0"}), Some("malformed_response")),
        (json!({"error_code":-1}), Some("device_rejected")),
        (json!({"error_code":0}), None),
    ] {
        let (port, peer) = peer(vec![info.clone(), info.clone(), ack]).await;
        let mut session = handshake_at("127.0.0.1", port, "test@example.invalid", "test-password")
            .await
            .unwrap();
        let result = crate::ops::tapo_toggle(&mut session).await;
        match expected {
            Some(code) => assert_eq!(crate::error::code(&result.unwrap_err()), code),
            None => assert!(result.unwrap()),
        }
        let requests = peer.await.unwrap();
        assert_eq!(requests.len(), 3);
        assert_eq!(
            requests[2],
            json!({"method":"set_device_info","params":{"device_on":true}})
        );
    }
}
#[tokio::test]
async fn repeated_energy_reads_reuse_the_authenticated_session() {
    let response = json!({"error_code":0,"result":{"current_power":2300,"today_energy":45,"month_energy":500}});
    let (port, peer) = peer(vec![response.clone(), response.clone()]).await;
    let mut session = handshake_at("127.0.0.1", port, "test@example.invalid", "test-password")
        .await
        .unwrap();
    for _ in 0..2 {
        assert_eq!(
            crate::ops::tapo_energy_usage(&mut session).await.unwrap(),
            response
        );
    }
    let requests = peer.await.unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests.iter().all(|r| r["method"] == "get_energy_usage"));
}

#[tokio::test]
async fn tampered_http_response_never_triggers_a_power_mutation() {
    let info = json!({"error_code":0,"result":{"model":"P125","hw_ver":"1.0","fw_ver":"1.0","rssi":-40,"device_on":false,"device_id":"synthetic-tapo"}});
    let (port, peer) = peer_with_corruption(vec![info], true).await;
    let mut session = handshake_at("127.0.0.1", port, "test@example.invalid", "test-password")
        .await
        .unwrap();
    let error = crate::ops::tapo_toggle(&mut session).await.unwrap_err();
    assert_eq!(crate::error::code(&error), "integrity_failed");
    assert_eq!(peer.await.unwrap().len(), 1);
}
