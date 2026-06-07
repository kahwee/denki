//! Network transport for legacy Kasa devices (port 9999).
//! Tapo/KLAP lives in klap.rs.

use crate::cipher;
use anyhow::Result;
use std::io::ErrorKind;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};

/// Port used by all legacy Kasa devices (KL135/LB130, KP115, HS series, etc.)
const PORT: u16 = 9999;

pub(crate) fn connect_timeout_error(addr: &str, seconds: u64) -> anyhow::Error {
    anyhow::anyhow!(
        "Timed out connecting to {addr} after {seconds}s. The device may be offline or unreachable."
    )
}

pub(crate) fn connect_error(addr: &str, err: &std::io::Error) -> anyhow::Error {
    let hint = match err.kind() {
        ErrorKind::ConnectionRefused
        | ErrorKind::HostUnreachable
        | ErrorKind::NetworkUnreachable
        | ErrorKind::NotConnected
        | ErrorKind::AddrNotAvailable => {
            "The device may be offline, unreachable, or on the wrong network."
        }
        ErrorKind::TimedOut => "The device may be offline or not responding.",
        _ => "The device may be offline or unreachable.",
    };
    anyhow::anyhow!("Could not connect to {addr}: {err}. {hint}")
}

pub async fn send(host: &str, payload: serde_json::Value) -> Result<serde_json::Value> {
    let addr = format!("{host}:{PORT}");
    let mut stream =
        tokio::time::timeout(std::time::Duration::from_secs(5), TcpStream::connect(&addr))
            .await
            .map_err(|_| connect_timeout_error(&addr, 5))?
            .map_err(|e| connect_error(&addr, &e))?;

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

/// Broadcast a sysinfo probe and call `f` for each device as it responds.
///
/// Sends to both 255.255.255.255 and all local subnet broadcasts — some routers
/// drop the limited broadcast, so subnet-directed ones (e.g. 192.168.4.255) are
/// needed to reach devices on specific VLANs. Deduplicates by IP in case a device
/// responds to both.
pub async fn broadcast_each<F>(timeout_secs: u64, mut f: F) -> Result<usize>
where
    F: FnMut(std::net::IpAddr, serde_json::Value),
{
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.set_broadcast(true)?;

    let probe = serde_json::json!({"system": {"get_sysinfo": {}}});
    let raw = serde_json::to_vec(&probe)?;
    let encoded = cipher::encode_raw(&raw);

    // Always send the limited broadcast.
    socket
        .send_to(&encoded, format!("255.255.255.255:{PORT}"))
        .await?;

    // Also send a directed broadcast on each local IPv4 subnet.
    // This reaches devices on subnets whose router drops 255.255.255.255.
    if let Ok(ifaces) = if_addrs::get_if_addrs() {
        for iface in ifaces {
            if let if_addrs::IfAddr::V4(v4) = iface.addr {
                if let Some(bcast) = v4.broadcast {
                    if bcast != std::net::Ipv4Addr::BROADCAST {
                        let _ = socket.send_to(&encoded, format!("{bcast}:{PORT}")).await;
                    }
                }
            }
        }
    }

    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(timeout_secs);
    let mut seen = std::collections::HashSet::new();
    let mut count = 0usize;
    let mut buf = vec![0u8; 16384];
    while let Ok(Ok((n, addr))) =
        tokio::time::timeout_at(deadline, socket.recv_from(&mut buf)).await
    {
        let ip = addr.ip();
        if !seen.insert(ip) {
            continue; // duplicate response to multiple broadcasts
        }
        let decoded = cipher::decode(&buf[..n]);
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&decoded) {
            f(ip, json);
            count += 1;
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_error_mentions_offline_and_addr() {
        let msg = connect_timeout_error("192.168.4.27:9999", 5).to_string();
        assert!(msg.contains("192.168.4.27:9999"), "{msg}");
        assert!(
            msg.contains("offline") || msg.contains("unreachable"),
            "{msg}"
        );
        assert!(msg.contains("5s"), "{msg}");
    }

    #[test]
    fn connect_error_mentions_offline_hint() {
        let err = std::io::Error::new(ErrorKind::HostUnreachable, "No route to host");
        let msg = connect_error("192.168.4.27:9999", &err).to_string();
        assert!(msg.contains("192.168.4.27:9999"), "{msg}");
        assert!(msg.contains("No route to host"), "{msg}");
        assert!(
            msg.contains("offline") || msg.contains("wrong network"),
            "{msg}"
        );
    }
}
