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
use reqwest::Client;
use sha1::{Digest as Sha1Digest, Sha1};
use sha2::Sha256;

type Aes128CbcEnc = cbc::Encryptor<Aes128>;
type Aes128CbcDec = cbc::Decryptor<Aes128>;

/// An active KLAP session. Holds the derived keys and HTTP client.
pub struct KlapSession {
    key: [u8; 16],
    iv_base: [u8; 12],
    sig: [u8; 28],
    seq: i32,
    cookie: String,
    client: Client,
    request_url: String,
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

/// Perform the KLAP handshake and return a ready-to-use session.
pub async fn handshake(host: &str, username: &str, password: &str) -> Result<KlapSession> {
    let ah = auth_hash(username, password);
    let base = format!("http://{host}:80/app");

    let mut local_seed = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut local_seed);

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    // ── Handshake 1 ──────────────────────────────────────────────────────────
    let resp = client
        .post(format!("{base}/handshake1"))
        .body(local_seed.to_vec())
        .send()
        .await?;

    if !resp.status().is_success() {
        bail!("Handshake 1 failed: HTTP {}", resp.status());
    }

    // Grab TP_SESSIONID from Set-Cookie header (cookie_store not required)
    let cookie = resp
        .headers()
        .get_all("set-cookie")
        .iter()
        .find_map(|v| {
            let s = v.to_str().ok()?;
            s.split(';')
                .next()
                .filter(|p| p.trim_start().starts_with("TP_SESSIONID="))
                .map(|p| p.trim().to_string())
        })
        .ok_or_else(|| anyhow::anyhow!("No TP_SESSIONID cookie from {host}"))?;

    let body = resp.bytes().await?;
    if body.len() < 48 {
        bail!("Handshake 1 response too short ({} bytes)", body.len());
    }

    let remote_seed: [u8; 16] = body[..16].try_into()?;
    let server_hash = &body[16..48];

    // Verify server proved it knows auth_hash
    let expected = sha256_multi(&[&local_seed, &remote_seed, &ah]);
    if expected != server_hash {
        bail!("Authentication failed for {host} — check TAPO_USER and TAPO_PASS");
    }

    // ── Handshake 2 ──────────────────────────────────────────────────────────
    let client_proof = sha256_multi(&[&remote_seed, &local_seed, &ah]);
    let resp2 = client
        .post(format!("{base}/handshake2"))
        .header("Cookie", &cookie)
        .body(client_proof.to_vec())
        .send()
        .await?;

    if !resp2.status().is_success() {
        bail!("Handshake 2 failed: HTTP {}", resp2.status());
    }

    // ── Key derivation ────────────────────────────────────────────────────────
    let key: [u8; 16] = sha256_multi(&[b"lsk", &local_seed, &remote_seed, &ah])[..16]
        .try_into()?;

    let iv_full = sha256_multi(&[b"iv", &local_seed, &remote_seed, &ah]);
    let iv_base: [u8; 12] = iv_full[..12].try_into()?;
    let seq = i32::from_be_bytes(iv_full[28..32].try_into()?);

    let sig: [u8; 28] = sha256_multi(&[b"ldk", &local_seed, &remote_seed, &ah])[..28]
        .try_into()?;

    Ok(KlapSession {
        key,
        iv_base,
        sig,
        seq,
        cookie,
        client,
        request_url: format!("{base}/request"),
    })
}

impl KlapSession {
    /// Encrypt a JSON string, POST it to the device, decrypt and return the response.
    pub async fn send(&mut self, json: &str) -> Result<serde_json::Value> {
        let (payload, seq) = self.encrypt(json.as_bytes())?;

        let resp = self
            .client
            .post(&self.request_url)
            .query(&[("seq", seq)])
            .header("Cookie", &self.cookie)
            .body(payload)
            .send()
            .await?;

        if !resp.status().is_success() {
            bail!("Request failed: HTTP {}", resp.status());
        }

        let bytes = resp.bytes().await?;
        let plaintext = self.decrypt(&bytes)?;
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
