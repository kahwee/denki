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

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    #[test]
    fn encode_raw_round_trips_with_decode() {
        let plaintext = br#"{"system":{"get_sysinfo":{}}}"#;
        assert_eq!(decode(&encode_raw(plaintext)), plaintext);
    }

    #[test]
    fn encode_strips_prefix_before_decode() {
        let plaintext = b"hello kasa";
        let framed = encode(plaintext);
        let len = u32::from_be_bytes(framed[..4].try_into().unwrap()) as usize;
        assert_eq!(len, plaintext.len());
        assert_eq!(decode(&framed[4..]), plaintext);
    }

    #[test]
    fn tcp_frame_is_four_bytes_longer_than_raw() {
        let plain = b"ping";
        assert_eq!(encode(plain).len(), encode_raw(plain).len() + 4);
    }

    // ── Known-plaintext table (manually derived, key=0xAB) ────────────────────
    //
    //  Input byte b, running key k:
    //    cipher = b ^ k;  next_k = cipher
    //
    //  ""    → []
    //  "1"   → [0x31^0xAB]                  = [0x9A]
    //  "12"  → [0x9A, 0x32^0x9A]            = [0x9A, 0xA8]
    //  "{"   → [0x7B^0xAB]                  = [0xD0]  ← first byte of every sysinfo probe
    //  "\x00"→ [0x00^0xAB]                  = [0xAB]

    #[rstest]
    #[case(&[][..],        &[][..])]
    #[case(b"1",           &[0x9A])]
    #[case(b"12",          &[0x9A, 0xA8])]
    #[case(b"{",           &[0xD0])]
    #[case(&[0x00],        &[0xAB])]
    fn encode_raw_known_output(#[case] input: &[u8], #[case] expected: &[u8]) {
        assert_eq!(encode_raw(input), expected);
    }

    #[test]
    fn tcp_frame_for_known_input_has_correct_prefix_and_body() {
        let framed = encode(b"12");
        assert_eq!(&framed[..4], &[0x00, 0x00, 0x00, 0x02]); // big-endian len = 2
        assert_eq!(&framed[4..], &[0x9A, 0xA8]);
    }

    proptest! {
        #[test]
        fn encode_raw_decode_round_trips_for_any_input(data in proptest::collection::vec(any::<u8>(), 0..=512)) {
            prop_assert_eq!(decode(&encode_raw(&data)), data);
        }

        #[test]
        fn tcp_length_prefix_always_matches_plaintext_len(data in proptest::collection::vec(any::<u8>(), 0..=512)) {
            let framed = encode(&data);
            let prefix_len = u32::from_be_bytes(framed[..4].try_into().unwrap()) as usize;
            prop_assert_eq!(prefix_len, data.len());
        }

        #[test]
        fn decode_is_inverse_of_encode_raw(data in proptest::collection::vec(any::<u8>(), 0..=512)) {
            // encode then decode is identity; also decode then encode is identity
            let ciphertext = encode_raw(&data);
            prop_assert_eq!(encode_raw(&decode(&ciphertext)), ciphertext);
        }
    }
}
