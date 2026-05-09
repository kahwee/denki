//! Network transport layer for the TP-Link Kasa legacy protocol.
//!
//! All devices on port 9999 use this protocol:
//!   - Discovery: UDP broadcast to 255.255.255.255:9999, listen for responses
//!   - Commands:  TCP connection to device_ip:9999, send/receive framed JSON
//!
//! Newer TP-Link devices (P125, L530, Tapo series) use a completely different
//! KLAP protocol on port 80 with SHA256 handshake + AES encryption. Not here.

use crate::cipher;
use anyhow::{Context, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};

/// Port used by all legacy Kasa devices (KL135, KP115, HS series, etc.)
const PORT: u16 = 9999;

/// Send a JSON command to a device over TCP and return the decoded JSON response.
///
/// Each call opens a fresh TCP connection. The device closes the socket after
/// responding, so connection reuse is not possible with this protocol.
///
/// Wire format (both directions):
///   [4 bytes big-endian length] [XOR-encrypted JSON body]
pub async fn send(host: &str, payload: serde_json::Value) -> Result<serde_json::Value> {
    let addr = format!("{host}:{PORT}");
    let mut stream = TcpStream::connect(&addr)
        .await
        .with_context(|| format!("Cannot connect to {addr}"))?;

    // Serialize to JSON, then XOR-encrypt with 4-byte length prefix for TCP
    let raw = serde_json::to_vec(&payload)?;
    let encoded = cipher::encode(&raw);
    stream.write_all(&encoded).await?;

    // Read exactly 4 bytes to learn how many cipher bytes follow
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;

    // Read exactly `len` cipher bytes, then decode
    let mut body = vec![0u8; len];
    stream.read_exact(&mut body).await?;

    let decoded = cipher::decode(&body);
    let response = serde_json::from_slice(&decoded)?;
    Ok(response)
}

/// Broadcast a `get_sysinfo` probe via UDP and collect all device responses
/// within `timeout_secs` seconds.
///
/// Returns a list of (IP address, raw sysinfo JSON) pairs, one per device.
///
/// Important differences from TCP:
/// - No length prefix in either direction — UDP datagrams are self-delimiting
/// - Devices respond to the source port, so we bind an ephemeral port
/// - Multiple devices may respond to a single broadcast; we read until timeout
///
/// Devices that don't respond (offline, wrong subnet, KLAP-only) are silently
/// skipped. Malformed responses are also silently dropped.
pub async fn broadcast(timeout_secs: u64) -> Result<Vec<(std::net::IpAddr, serde_json::Value)>> {
    // Bind on all interfaces, ephemeral port — OS assigns the source port
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.set_broadcast(true)?;

    // Standard sysinfo probe — all Kasa devices respond to this
    let probe = serde_json::json!({"system": {"get_sysinfo": {}}});

    // UDP: encode_raw (no length prefix). Using encode() here adds 4 garbage
    // bytes that devices would try to decrypt as cipher text.
    let raw = serde_json::to_vec(&probe)?;
    let encoded = cipher::encode_raw(&raw);
    socket
        .send_to(&encoded, format!("255.255.255.255:{PORT}"))
        .await?;

    let mut results = Vec::new();
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(timeout_secs);

    let mut buf = vec![0u8; 4096];
    // Stop on timeout or socket error; silently drop malformed responses
    while let Ok(Ok((n, addr))) =
        tokio::time::timeout_at(deadline, socket.recv_from(&mut buf)).await
    {
        // UDP responses have no length prefix — decode raw from byte 0
        let decoded = cipher::decode(&buf[..n]);
        // Silently skip any response that isn't valid JSON (e.g. KLAP devices)
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&decoded) {
            results.push((addr.ip(), json));
        }
    }

    Ok(results)
}
