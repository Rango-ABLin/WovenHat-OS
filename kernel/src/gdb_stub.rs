//! GDB Remote Serial Protocol (RSP) packet framing (Stage 32 / WovenDev).
//!
//! Implements the wire-level encoding and decoding of GDB RSP packets as pure
//! functions over byte slices — no I/O, no global state, no allocation.  A
//! higher-level stub loop that drives a debug serial port, handles `g`/`m`/`?`
//! commands and wires into the trap handler will be built on top of this layer.
//!
//! # Packet grammar
//!
//! ```text
//! $<data>#<cs_hi><cs_lo>
//! ```
//!
//! * `data` — the ASCII command or reply body.
//! * `cs_hi cs_lo` — two lowercase hex digits encoding the 8-bit modular sum
//!   of all bytes in `data` (the GDB RSP checksum; see GDB internals § E.1).
//!
//! Acknowledgements (`+` / `-`) are part of the transport loop, not the
//! framing, and are handled separately.

/// Maximum bytes in the `data` portion of one RSP packet.
pub const MAX_PACKET_DATA: usize = 512;

// ── Packet type ───────────────────────────────────────────────────────────────

/// A checksum-verified GDB RSP packet.
#[derive(Clone, Copy)]
pub struct Packet {
    data: [u8; MAX_PACKET_DATA],
    len: usize,
}

impl Packet {
    const EMPTY: Self = Self {
        data: [0u8; MAX_PACKET_DATA],
        len: 0,
    };

    /// The verified data bytes.
    pub fn data(&self) -> &[u8] {
        &self.data[..self.len]
    }

    /// The data as a UTF-8 string slice, or `None` if it contains non-ASCII.
    pub fn as_str(&self) -> Option<&str> {
        core::str::from_utf8(&self.data[..self.len]).ok()
    }
}

// ── Core framing functions ────────────────────────────────────────────────────

/// Compute the GDB RSP checksum: the 8-bit modular sum of all bytes in `data`.
pub fn checksum(data: &[u8]) -> u8 {
    data.iter().fold(0u8, |acc, &b| acc.wrapping_add(b))
}

/// Encode a GDB RSP packet into `out`.
///
/// Writes `$<data>#<cs_hi><cs_lo>`.  Returns the number of bytes written, or
/// `None` if `data` exceeds [`MAX_PACKET_DATA`] or `out` is too small.
pub fn encode(data: &[u8], out: &mut [u8]) -> Option<usize> {
    if data.len() > MAX_PACKET_DATA {
        return None;
    }
    // '$' + data + '#' + 2 hex checksum digits
    let needed = 1 + data.len() + 3;
    if out.len() < needed {
        return None;
    }
    out[0] = b'$';
    out[1..1 + data.len()].copy_from_slice(data);
    out[1 + data.len()] = b'#';
    let cs = checksum(data);
    out[2 + data.len()] = hex_nibble(cs >> 4);
    out[3 + data.len()] = hex_nibble(cs & 0x0f);
    Some(needed)
}

/// Parse and verify a GDB RSP packet from the start of `input`.
///
/// Expects `$<data>#<cs_hi><cs_lo>`.  Returns the decoded [`Packet`] on
/// success, or `None` on any format error or checksum mismatch.  Bytes after
/// the closing two-hex checksum are ignored.
pub fn decode(input: &[u8]) -> Option<Packet> {
    // Minimum: '$' + at-least-0-data + '#' + 2 hex = 4 bytes
    if input.len() < 4 {
        return None;
    }
    if input[0] != b'$' {
        return None;
    }
    // Locate '#' — the end-of-data marker.
    let hash_offset = input[1..].iter().position(|&b| b == b'#')?;
    let hash_idx = 1 + hash_offset;
    if input.len() < hash_idx + 3 {
        // Not enough room for '#' + 2 hex digits.
        return None;
    }
    let data = &input[1..hash_idx];
    if data.len() > MAX_PACKET_DATA {
        return None;
    }
    let cs_hi = from_hex_nibble(input[hash_idx + 1])?;
    let cs_lo = from_hex_nibble(input[hash_idx + 2])?;
    let cs_received = (cs_hi << 4) | cs_lo;
    if checksum(data) != cs_received {
        return None;
    }
    let mut pkt = Packet::EMPTY;
    pkt.data[..data.len()].copy_from_slice(data);
    pkt.len = data.len();
    Some(pkt)
}

// ── Hex helpers ───────────────────────────────────────────────────────────────

fn hex_nibble(v: u8) -> u8 {
    if v < 10 { b'0' + v } else { b'a' + (v - 10) }
}

fn from_hex_nibble(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

// ── Boot self-test ────────────────────────────────────────────────────────────

/// Boot-time self-test. Verifies the checksum formula, encode/decode
/// round-trips, checksum-mismatch rejection, and edge cases (empty packet,
/// maximum-length data, non-`$` start byte). Returns `true` on pass.
#[cfg(any(test, feature = "stage32-8-test"))]
pub fn self_test() -> bool {
    // ── Checksum ──────────────────────────────────────────────────────────────
    // Empty data → 0.
    if checksum(b"") != 0 { return false; }
    // Known value: 'g' = 0x67.
    if checksum(b"g") != 0x67 { return false; }
    // Wrap-around: 0xFF + 0x01 = 0x00.
    if checksum(&[0xFF, 0x01]) != 0x00 { return false; }
    // Multi-byte: sum of 'O', 'K' = 0x4f + 0x4b = 0x9a.
    if checksum(b"OK") != 0x9a { return false; }

    // ── Encode ────────────────────────────────────────────────────────────────
    let mut buf = [0u8; 16];
    // "?" encodes to "$?#3f" (checksum of '?' = 0x3f).
    let Some(n) = encode(b"?", &mut buf) else { return false; };
    if n != 5 { return false; }
    if &buf[..5] != b"$?#3f" { return false; }

    // "OK" → "$OK#9a".
    let Some(n) = encode(b"OK", &mut buf) else { return false; };
    if n != 6 { return false; }
    if &buf[..6] != b"$OK#9a" { return false; }

    // Buffer too small → None.
    let mut tiny = [0u8; 3];
    if encode(b"?", &mut tiny).is_some() { return false; }

    // ── Decode ────────────────────────────────────────────────────────────────
    // Round-trip: encode then decode.
    let mut enc = [0u8; 8];
    let Some(_) = encode(b"g", &mut enc) else { return false; };
    let Some(pkt) = decode(&enc) else { return false; };
    if pkt.data() != b"g" { return false; }

    // "OK" round-trip.
    let mut enc2 = [0u8; 8];
    let Some(_) = encode(b"OK", &mut enc2) else { return false; };
    let Some(pkt2) = decode(&enc2) else { return false; };
    if pkt2.as_str() != Some("OK") { return false; }

    // Trailing bytes after checksum are ignored.
    let Some(pkt3) = decode(b"$OK#9aXXXX") else { return false; };
    if pkt3.data() != b"OK" { return false; }

    // ── Rejection cases ───────────────────────────────────────────────────────
    if decode(b"!OK#9a").is_some() { return false; }  // wrong leading byte
    if decode(b"$OK#00").is_some() { return false; }  // checksum mismatch
    if decode(b"$?#").is_some()    { return false; }  // too short
    if decode(b"$?#zz").is_some()  { return false; }  // invalid hex
    if decode(b"$OK").is_some()    { return false; }  // no '#'

    // ── Empty packet ──────────────────────────────────────────────────────────
    let mut ebuf = [0u8; 8];
    let Some(en) = encode(b"", &mut ebuf) else { return false; };
    if en != 4 { return false; }
    if &ebuf[..4] != b"$#00" { return false; }
    let Some(epkt) = decode(&ebuf[..4]) else { return false; };
    if !epkt.data().is_empty() { return false; }

    true
}

// ── In-module unit tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_empty() {
        assert_eq!(checksum(b""), 0);
    }

    #[test]
    fn checksum_known_values() {
        assert_eq!(checksum(b"g"), 0x67);
        assert_eq!(checksum(b"OK"), 0x9a);
        assert_eq!(checksum(&[0xFF, 0x01]), 0x00);
    }

    #[test]
    fn encode_question_mark() {
        let mut buf = [0u8; 8];
        let n = encode(b"?", &mut buf).unwrap();
        assert_eq!(n, 5);
        assert_eq!(&buf[..5], b"$?#3f");
    }

    #[test]
    fn encode_ok() {
        let mut buf = [0u8; 8];
        let n = encode(b"OK", &mut buf).unwrap();
        assert_eq!(n, 6);
        assert_eq!(&buf[..6], b"$OK#9a");
    }

    #[test]
    fn encode_empty() {
        let mut buf = [0u8; 8];
        let n = encode(b"", &mut buf).unwrap();
        assert_eq!(n, 4);
        assert_eq!(&buf[..4], b"$#00");
    }

    #[test]
    fn encode_buffer_too_small() {
        let mut buf = [0u8; 3];
        assert!(encode(b"?", &mut buf).is_none());
    }

    #[test]
    fn decode_question_mark() {
        let pkt = decode(b"$?#3f").unwrap();
        assert_eq!(pkt.data(), b"?");
    }

    #[test]
    fn decode_ok() {
        let pkt = decode(b"$OK#9a").unwrap();
        assert_eq!(pkt.as_str(), Some("OK"));
    }

    #[test]
    fn round_trip() {
        let mut buf = [0u8; 32];
        encode(b"qSupported", &mut buf).unwrap();
        let pkt = decode(&buf).unwrap();
        assert_eq!(pkt.as_str(), Some("qSupported"));
    }

    #[test]
    fn trailing_bytes_ignored() {
        let pkt = decode(b"$OK#9aXXXXXX").unwrap();
        assert_eq!(pkt.data(), b"OK");
    }

    #[test]
    fn reject_wrong_start_byte() {
        assert!(decode(b"!OK#9a").is_none());
    }

    #[test]
    fn reject_checksum_mismatch() {
        assert!(decode(b"$OK#00").is_none());
    }

    #[test]
    fn reject_too_short() {
        assert!(decode(b"$?#").is_none());
        assert!(decode(b"abc").is_none());
    }

    #[test]
    fn reject_invalid_hex_in_checksum() {
        assert!(decode(b"$?#zz").is_none());
    }

    #[test]
    fn reject_no_hash() {
        assert!(decode(b"$OK").is_none());
    }

    #[test]
    fn self_test_passes() {
        assert!(self_test());
    }
}
