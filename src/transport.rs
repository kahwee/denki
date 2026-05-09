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

/// Send a UDP sysinfo broadcast and call `f` for each device response as it arrives.
///
/// Returns the total number of valid responses received.
/// Callers receive results immediately rather than waiting for the full timeout to elapse.
///
/// Important differences from TCP:
/// - No length prefix in either direction — UDP datagrams are self-delimiting
/// - Devices respond to the source port, so we bind an ephemeral port
/// - Multiple devices may respond to a single broadcast; we read until timeout
///
/// Devices that don't respond (offline, wrong subnet, KLAP-only) are silently
/// skipped. Malformed responses are also silently dropped.
pub async fn broadcast_each<F>(timeout_secs: u64, mut f: F) -> Result<usize>
where
    F: FnMut(std::net::IpAddr, serde_json::Value),
{
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.set_broadcast(true)?;

    let probe = serde_json::json!({"system": {"get_sysinfo": {}}});
    let raw = serde_json::to_vec(&probe)?;
    let encoded = cipher::encode_raw(&raw);
    socket
        .send_to(&encoded, format!("255.255.255.255:{PORT}"))
        .await?;

    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(timeout_secs);
    let mut count = 0usize;
    let mut buf = vec![0u8; 4096];
    while let Ok(Ok((n, addr))) =
        tokio::time::timeout_at(deadline, socket.recv_from(&mut buf)).await
    {
        let decoded = cipher::decode(&buf[..n]);
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&decoded) {
            f(addr.ip(), json);
            count += 1;
        }
    }
    Ok(count)
}

/// Broadcast a `get_sysinfo` probe and collect all responses within `timeout_secs`.
/// Returns a list of (IP, sysinfo JSON) pairs. Prefer `broadcast_each` when streaming output.
pub async fn broadcast(timeout_secs: u64) -> Result<Vec<(std::net::IpAddr, serde_json::Value)>> {
    let mut results = Vec::new();
    broadcast_each(timeout_secs, |ip, json| results.push((ip, json))).await?;
    Ok(results)
}
