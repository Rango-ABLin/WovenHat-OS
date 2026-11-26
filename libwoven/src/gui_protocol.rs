//! Bounded, opt-in additions to the shell's application window protocol.
//!
//! Legacy clients never see these tags: the shell sends `WKEY2` key events
//! only to windows that opt in with [`OPT_IN`], and it ignores unknown client
//! messages. `WREQ` lets an opted-in client make a small window-manager
//! request; the shell still authenticates the sender as the surface owner.

/// Client-to-shell opt-in, sent once after the `WOPN`/`WCAP` handshake.
pub const OPT_IN: &[u8; 5] = b"WKV2\x01";
/// Shell-to-client extended key event tag.
pub const KEY: &[u8; 5] = b"WKEY2";
/// Client-to-shell window-manager request tag.
pub const REQUEST: &[u8; 4] = b"WREQ";

/// Extended key codes. ASCII characters use [`ASCII`] and carry the byte in
/// the final slot; the other codes leave that slot zero.
pub mod key {
    pub const ASCII: u8 = 0;
    pub const ENTER: u8 = 1;
    pub const BACKSPACE: u8 = 2;
    pub const TAB: u8 = 3;
}

/// Window-manager request actions and their bounds.
pub mod request {
    pub const SET_TITLE: u8 = 1;
    pub const FOCUS: u8 = 2;
    pub const MAX_TITLE: usize = 24;
}

/// Encode one extended key event. Returns the used length of `output`.
pub fn encode_key(code: u8, ascii: Option<u8>, output: &mut [u8; 7]) -> usize {
    output[..5].copy_from_slice(KEY);
    output[5] = code;
    output[6] = if code == key::ASCII {
        ascii.unwrap_or(0)
    } else {
        0
    };
    output.len()
}

/// Decode an extended key event. Unknown codes and malformed lengths are
/// rejected so a stray message cannot drive client state.
pub fn decode_key(message: &[u8]) -> Option<(u8, Option<u8>)> {
    if message.len() != 7 || &message[..5] != KEY {
        return None;
    }
    match message[5] {
        key::ASCII => Some((key::ASCII, Some(message[6]))),
        key::ENTER => Some((key::ENTER, None)),
        key::BACKSPACE => Some((key::BACKSPACE, None)),
        key::TAB => Some((key::TAB, None)),
        _ => None,
    }
}

/// Encode a window-manager request into `output`, or return `None` when the
/// action or payload is out of bounds.
pub fn encode_request(action: u8, payload: &[u8], output: &mut [u8; 29]) -> Option<usize> {
    output[..4].copy_from_slice(REQUEST);
    output[4] = action;
    match action {
        request::SET_TITLE if payload.len() <= request::MAX_TITLE => {
            output[5..5 + payload.len()].copy_from_slice(payload);
            Some(5 + payload.len())
        }
        request::FOCUS if payload.is_empty() => Some(5),
        _ => None,
    }
}

/// Decode a window-manager request. The caller still has to authenticate the
/// sender as the owner of a window before acting on the result.
pub fn decode_request(message: &[u8]) -> Option<(u8, &[u8])> {
    if message.len() < 5 || &message[..4] != REQUEST {
        return None;
    }
    match message[4] {
        request::SET_TITLE if message.len() - 5 <= request::MAX_TITLE => {
            Some((request::SET_TITLE, &message[5..]))
        }
        request::FOCUS if message.len() == 5 => Some((request::FOCUS, &[])),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{decode_key, decode_request, encode_key, encode_request, key, request};

    #[test]
    fn key_round_trips_ascii_and_controls() {
        let mut bytes = [0u8; 7];
        let length = encode_key(key::ASCII, Some(b'x'), &mut bytes);
        assert_eq!(decode_key(&bytes[..length]), Some((key::ASCII, Some(b'x'))));
        let length = encode_key(key::TAB, None, &mut bytes);
        assert_eq!(decode_key(&bytes[..length]), Some((key::TAB, None)));
        let length = encode_key(key::ENTER, None, &mut bytes);
        assert_eq!(decode_key(&bytes[..length]), Some((key::ENTER, None)));
    }

    #[test]
    fn key_rejects_malformed_messages() {
        let mut bytes = [0u8; 7];
        encode_key(key::ENTER, None, &mut bytes);
        assert_eq!(decode_key(&bytes[..6]), None);
        bytes[5] = 99;
        assert_eq!(decode_key(&bytes), None);
        bytes[..5].copy_from_slice(b"NOPE!");
        assert_eq!(decode_key(&bytes), None);
    }

    #[test]
    fn request_round_trips_title_and_focus() {
        let mut bytes = [0u8; 29];
        let length = encode_request(request::SET_TITLE, b"pwd ok", &mut bytes).unwrap();
        assert_eq!(
            decode_request(&bytes[..length]),
            Some((request::SET_TITLE, b"pwd ok".as_slice()))
        );
        let length = encode_request(request::FOCUS, &[], &mut bytes).unwrap();
        assert_eq!(
            decode_request(&bytes[..length]),
            Some((request::FOCUS, &[][..]))
        );
    }

    #[test]
    fn request_rejects_out_of_bounds_payloads() {
        let mut bytes = [0u8; 29];
        assert_eq!(
            encode_request(request::SET_TITLE, &[b'x'; 25], &mut bytes),
            None
        );
        assert_eq!(encode_request(request::FOCUS, b"x", &mut bytes), None);
        assert_eq!(encode_request(99, &[], &mut bytes), None);
        let mut oversized = [0u8; 31];
        oversized[..5].copy_from_slice(b"WREQ\x01");
        assert_eq!(decode_request(&oversized), None);
    }
}
