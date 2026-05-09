//! XOR autokey cipher used by TP-Link's legacy Kasa protocol (port 9999).
//!
//! All communication is JSON encrypted with this cipher. The key starts at
//! 171 (0xAB) and auto-chains: each output byte becomes the key for the next.
//!
//! Encrypt:  c = p ^ key;  key = c
//! Decrypt:  p = c ^ key;  key = c
//!
//! TCP frames include a 4-byte big-endian length prefix before the cipher body.
//! UDP packets do NOT include the length prefix — this is a common source of bugs.

/// Encode with a 4-byte big-endian length prefix, for use over TCP.
///
/// Frame layout: [u32 length][XOR-encrypted body]
pub fn encode(plaintext: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(plaintext.len() + 4);
    // Big-endian byte count of the plaintext, prepended so the receiver
    // knows exactly how many bytes to read before decoding.
    out.extend_from_slice(&(plaintext.len() as u32).to_be_bytes());
    out.extend_from_slice(&encode_raw(plaintext));
    out
}

/// Encode without any length prefix, for use in UDP broadcast discovery.
///
/// UDP already knows the packet boundary from the datagram, so no prefix needed.
/// Sending a length prefix in UDP would cause the first 4 bytes to be decoded
/// as cipher data, producing garbage.
pub fn encode_raw(plaintext: &[u8]) -> Vec<u8> {
    let mut key: u8 = 171; // 0xAB — fixed starting key defined by TP-Link
    let mut out = Vec::with_capacity(plaintext.len());
    for &b in plaintext {
        let c = b ^ key;
        key = c; // autokey: each encrypted byte feeds the next round
        out.push(c);
    }
    out
}

/// Decode cipher bytes (no length prefix expected — strip it before calling).
///
/// Used for both TCP body (after reading the 4-byte length) and UDP responses
/// (which arrive without a length prefix at all).
pub fn decode(ciphertext: &[u8]) -> Vec<u8> {
    let mut key: u8 = 171;
    let mut out = Vec::with_capacity(ciphertext.len());
    for &b in ciphertext {
        out.push(b ^ key);
        key = b; // autokey: each ciphertext byte feeds the next round
    }
    out
}
