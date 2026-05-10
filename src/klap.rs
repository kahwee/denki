//! KLAP protocol — transport for Tapo smart devices (P125, etc.)
//!
//! Tapo devices on port 80 use a two-phase handshake to establish an
//! AES-128-CBC encrypted session. All communication is over plain HTTP.
//!
//! Auth hash (KlapTransportV2, used by current Tapo firmware):
//!   auth_hash = SHA256(SHA1(username) + SHA1(password))
//!
//! Handshake 1 — POST /app/handshake1:
//!   send:    local_seed  (16 random bytes)
//!   receive: remote_seed (16 bytes) | server_hash (32 bytes)
//!   verify:  SHA256(local_seed + remote_seed + auth_hash) == server_hash
//!   save:    TP_SESSIONID cookie from response headers
//!
//! Handshake 2 — POST /app/handshake2 (with session cookie):
//!   send:    SHA256(remote_seed + local_seed + auth_hash)
//!   receive: HTTP 200 on success
//!
//! Key derivation (all SHA256 over prefixed seeds):
//!   key     = SHA256(b"lsk" + local_seed + remote_seed + auth_hash)[0..16]
//!   iv_full = SHA256(b"iv"  + local_seed + remote_seed + auth_hash)
//!   iv_base = iv_full[0..12]
//!   seq     = i32::from_be_bytes(iv_full[28..32])  (initial, increments per call)
//!   sig     = SHA256(b"ldk" + local_seed + remote_seed + auth_hash)[0..28]
//!
//! Per-request encryption — POST /app/request?seq={seq}:
//!   seq    += 1
//!   iv      = iv_base + seq.to_be_bytes()  (16 bytes total)
//!   cipher  = AES-128-CBC-PKCS7(key, iv, plaintext_json)
//!   sig_tag = SHA256(sig + seq.to_be_bytes() + ciphertext)[0..32]
//!   body    = sig_tag + ciphertext
//!
//! Response decryption:
//!   skip first 32 bytes (signature), AES-128-CBC-PKCS7 decrypt the rest

use aes::Aes128;
use anyhow::{bail, Result};
use cbc::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use rand::RngCore;
use sha1::{Digest as Sha1Digest, Sha1};
use sha2::Sha256;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{timeout, Duration};

/// Timeout for each KLAP TCP connect + read + write operation.
const KLAP_TIMEOUT: Duration = Duration::from_secs(10);

type Aes128CbcEnc = cbc::Encryptor<Aes128>;
type Aes128CbcDec = cbc::Decryptor<Aes128>;

/// An active KLAP session. Holds the derived keys and connection info.
pub struct KlapSession {
    key: [u8; 16],
    iv_base: [u8; 12],
    sig: [u8; 28],
    seq: i32,
    cookie: String,
    host: String,
}

fn sha1_of(data: &[u8]) -> [u8; 20] {
    let mut h = Sha1::new();
    h.update(data);
    h.finalize().into()
}

fn sha256_of(data: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(data);
    h.finalize().into()
}

fn sha256_multi(parts: &[&[u8]]) -> [u8; 32] {
    let mut h = Sha256::new();
    for p in parts {
        h.update(p);
    }
    h.finalize().into()
}

/// Generate the auth hash from Tapo account credentials.
/// Uses KlapTransportV2 format: SHA256(SHA1(username) + SHA1(password))
pub fn auth_hash(username: &str, password: &str) -> [u8; 32] {
    let un = sha1_of(username.as_bytes());
    let pw = sha1_of(password.as_bytes());
    sha256_of(&[un.as_slice(), pw.as_slice()].concat())
}

/// Read bytes from stream until the pattern `\r\n\r\n` is found.
/// Returns all bytes read (including the separator).
async fn read_headers(stream: &mut TcpStream) -> Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(512);
    loop {
        let b = stream.read_u8().await?;
        buf.push(b);
        if buf.ends_with(b"\r\n\r\n") {
            return Ok(buf);
        }
        if buf.len() > 8192 {
            bail!("HTTP response headers too large");
        }
    }
}

/// Send a raw HTTP POST over a fresh TCP connection, returns (status, headers, body).
async fn http_post(
    host: &str,
    path: &str,
    extra_headers: &[(&str, &str)],
    body: &[u8],
) -> Result<(u16, String, Vec<u8>)> {
    let mut stream = timeout(KLAP_TIMEOUT, TcpStream::connect(format!("{host}:80")))
        .await
        .map_err(|_| anyhow::anyhow!("Timed out connecting to {host}:80"))?
        .map_err(|e| anyhow::anyhow!("Cannot connect to {host}:80: {e}"))?;
    stream.set_nodelay(true)?;

    let mut req = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\n",
        body.len()
    );
    for (k, v) in extra_headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("\r\n");

    timeout(KLAP_TIMEOUT, stream.write_all(req.as_bytes()))
        .await
        .map_err(|_| anyhow::anyhow!("Timed out sending request to {host}"))??;
    timeout(KLAP_TIMEOUT, stream.write_all(body))
        .await
        .map_err(|_| anyhow::anyhow!("Timed out sending request body to {host}"))??;

    // Read headers byte by byte until \r\n\r\n
    let header_bytes = timeout(KLAP_TIMEOUT, read_headers(&mut stream))
        .await
        .map_err(|_| anyhow::anyhow!("Timed out reading response headers from {host}"))??;
    let headers_str = String::from_utf8_lossy(&header_bytes).into_owned();

    // Parse status code from first line: "HTTP/1.1 200 OK"
    let status: u16 = headers_str
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| anyhow::anyhow!("Could not parse HTTP status"))?;

    // Parse Content-Length to read exactly that many body bytes.
    // Absent on responses with no body (e.g. handshake2 200 OK); treat those as 0.
    let content_length: usize = headers_str
        .lines()
        .find_map(|l| {
            if l.to_lowercase().starts_with("content-length:") {
                l[15..].trim().parse().ok()
            } else {
                None
            }
        })
        .unwrap_or(0);

    let mut resp_body = vec![0u8; content_length];
    if content_length > 0 {
        timeout(KLAP_TIMEOUT, stream.read_exact(&mut resp_body))
            .await
            .map_err(|_| anyhow::anyhow!("Timed out reading response body from {host}"))??;
    }

    Ok((status, headers_str, resp_body))
}

/// Perform the KLAP handshake and return a ready-to-use session.
pub async fn handshake(host: &str, username: &str, password: &str) -> Result<KlapSession> {
    let ah = auth_hash(username, password);

    let mut local_seed = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut local_seed);

    // ── Handshake 1 ──────────────────────────────────────────────────────────
    let (status1, headers1, body1) = http_post(host, "/app/handshake1", &[], &local_seed).await?;
    if status1 != 200 {
        bail!("Handshake 1 failed: HTTP {status1}");
    }

    // Grab TP_SESSIONID from Set-Cookie header
    let cookie = headers1
        .lines()
        .find_map(|line| {
            let lower = line.to_lowercase();
            if !lower.starts_with("set-cookie:") {
                return None;
            }
            let value = line[11..].trim();
            value
                .split(';')
                .next()
                .filter(|p| p.trim_start().to_lowercase().starts_with("tp_sessionid="))
                .map(|p| p.trim().to_string())
        })
        .ok_or_else(|| anyhow::anyhow!("No TP_SESSIONID cookie from {host}"))?;

    if body1.len() < 48 {
        bail!("Handshake 1 response too short ({} bytes)", body1.len());
    }

    let remote_seed: [u8; 16] = body1[..16].try_into()?;
    let server_hash = &body1[16..48];

    // Verify server proved it knows auth_hash
    let expected = sha256_multi(&[&local_seed, &remote_seed, &ah]);
    if expected != server_hash {
        bail!("Authentication failed for {host} — check TAPO_USER and TAPO_PASS");
    }

    // ── Handshake 2 ──────────────────────────────────────────────────────────
    let client_proof = sha256_multi(&[&remote_seed, &local_seed, &ah]);
    let (status2, _, _) = http_post(
        host,
        "/app/handshake2",
        &[("Cookie", &cookie)],
        &client_proof,
    )
    .await?;
    if status2 != 200 {
        bail!("Handshake 2 failed: HTTP {status2}");
    }

    // ── Key derivation ────────────────────────────────────────────────────────
    let key: [u8; 16] = sha256_multi(&[b"lsk", &local_seed, &remote_seed, &ah])[..16].try_into()?;

    let iv_full = sha256_multi(&[b"iv", &local_seed, &remote_seed, &ah]);
    let iv_base: [u8; 12] = iv_full[..12].try_into()?;
    let seq = i32::from_be_bytes(iv_full[28..32].try_into()?);

    let sig: [u8; 28] = sha256_multi(&[b"ldk", &local_seed, &remote_seed, &ah])[..28].try_into()?;

    Ok(KlapSession {
        key,
        iv_base,
        sig,
        seq,
        cookie,
        host: host.to_string(),
    })
}

impl KlapSession {
    /// Encrypt a JSON string, POST it to the device, decrypt and return the response.
    pub async fn send(&mut self, json: &str) -> Result<serde_json::Value> {
        let (payload, seq) = self.encrypt(json.as_bytes())?;
        let path = format!("/app/request?seq={seq}");
        let cookie = self.cookie.clone();
        let host = self.host.clone();

        let (status, _, resp_body) =
            http_post(&host, &path, &[("Cookie", &cookie)], &payload).await?;

        if status != 200 {
            bail!("Request failed: HTTP {status}");
        }
        if resp_body.is_empty() {
            bail!("Tapo request returned empty body (missing Content-Length in response)");
        }

        let plaintext = self.decrypt(&resp_body)?;
        Ok(serde_json::from_str(&plaintext)?)
    }

    fn iv_for_seq(&self, seq: i32) -> [u8; 16] {
        let mut iv = [0u8; 16];
        iv[..12].copy_from_slice(&self.iv_base);
        iv[12..].copy_from_slice(&seq.to_be_bytes());
        iv
    }

    fn encrypt(&mut self, plaintext: &[u8]) -> Result<(Vec<u8>, i32)> {
        self.seq = self.seq.wrapping_add(1);
        let iv = self.iv_for_seq(self.seq);

        let ciphertext = Aes128CbcEnc::new(&self.key.into(), &iv.into())
            .encrypt_padded_vec_mut::<Pkcs7>(plaintext);

        let sig_tag = sha256_multi(&[&self.sig, &self.seq.to_be_bytes(), &ciphertext]);

        let mut payload = Vec::with_capacity(32 + ciphertext.len());
        payload.extend_from_slice(&sig_tag);
        payload.extend_from_slice(&ciphertext);

        Ok((payload, self.seq))
    }

    fn decrypt(&self, data: &[u8]) -> Result<String> {
        if data.len() < 32 {
            bail!("Response too short to decrypt ({} bytes)", data.len());
        }
        let iv = self.iv_for_seq(self.seq);
        let plaintext = Aes128CbcDec::new(&self.key.into(), &iv.into())
            .decrypt_padded_vec_mut::<Pkcs7>(&data[32..])
            .map_err(|e| anyhow::anyhow!("AES decrypt error: {e:?}"))?;
        Ok(String::from_utf8(plaintext)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn klap_timeout_is_positive() {
        assert!(KLAP_TIMEOUT.as_secs() > 0);
    }

    #[test]
    fn auth_hash_is_32_bytes() {
        let h = auth_hash("user@example.com", "secret");
        assert_eq!(h.len(), 32);
    }

    #[test]
    fn auth_hash_differs_by_credential() {
        let h1 = auth_hash("user@example.com", "pass1");
        let h2 = auth_hash("user@example.com", "pass2");
        assert_ne!(h1, h2);
    }

    #[test]
    fn auth_hash_is_deterministic() {
        let h1 = auth_hash("u@x.com", "pw");
        let h2 = auth_hash("u@x.com", "pw");
        assert_eq!(h1, h2);
    }
}
