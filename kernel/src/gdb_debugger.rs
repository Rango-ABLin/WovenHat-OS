//! Bounded GDB command dispatch and x86-64 register-context encoding.
//!
//! This module is deliberately pure: it owns no trap state, does not resume
//! execution, and never dereferences an arbitrary guest address. The live
//! debugger loop will bind these replies to a stopped task in a later slice.

/// Number of u64 registers in the fixed x86-64 remote register context.
pub const REGISTER_COUNT: usize = 24;
/// Maximum bytes returned by one bounded `m` memory command.
pub const MAX_MEMORY_READ: usize = 256;
pub const REGISTER_HEX_BYTES: usize = REGISTER_COUNT * 16;

/// Register order used by the bounded remote context.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RegisterContext {
    pub values: [u64; REGISTER_COUNT],
}

impl RegisterContext {
    pub const ZERO: Self = Self {
        values: [0; REGISTER_COUNT],
    };

    /// Encode all registers as target-endian bytes represented by lowercase hex.
    pub fn encode_hex(&self, out: &mut [u8]) -> Option<usize> {
        let needed = REGISTER_COUNT * 16;
        if out.len() < needed {
            return None;
        }
        let mut cursor = 0;
        for value in self.values {
            for byte in value.to_le_bytes() {
                out[cursor] = hex(byte >> 4);
                out[cursor + 1] = hex(byte & 0x0f);
                cursor += 2;
            }
        }
        Some(cursor)
    }
}

/// The bounded action a decoded debugger command requests.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Reply,
    Continue,
    Step,
    Kill,
    Empty,
}

/// Apply a complete fixed-size GDB `G` register write to caller-owned state.
pub fn write_registers(
    command: &[u8],
    context: &mut RegisterContext,
    reply: &mut [u8],
) -> Option<usize> {
    let data = command.strip_prefix(b"G")?;
    if data.len() != REGISTER_HEX_BYTES || reply.len() < 2 {
        return None;
    }
    let mut values = [0u64; REGISTER_COUNT];
    for (index, value) in values.iter_mut().enumerate() {
        let start = index * 16;
        let mut bytes = [0u8; 8];
        for (byte_index, byte) in bytes.iter_mut().enumerate() {
            let high = from_hex(data[start + byte_index * 2])?;
            let low = from_hex(data[start + byte_index * 2 + 1])?;
            *byte = (high << 4) | low;
        }
        *value = u64::from_le_bytes(bytes);
    }
    context.values = values;
    reply[..2].copy_from_slice(b"OK");
    Some(2)
}

/// Dispatch one decoded RSP command into caller-owned reply storage.
///
/// `memory_base` and `memory` describe the only readable window. A command
/// outside that window, or above [`MAX_MEMORY_READ`], returns `E01` and never
/// performs pointer arithmetic on an untrusted address.
pub fn dispatch(
    command: &[u8],
    context: &RegisterContext,
    memory_base: u64,
    memory: &[u8],
    reply: &mut [u8],
) -> Option<(Action, usize)> {
    if command == b"?" {
        return reply_bytes(reply, b"S05");
    }
    if command == b"g" {
        return context
            .encode_hex(reply)
            .map(|written| (Action::Reply, written));
    }
    if command == b"c" {
        return Some((Action::Continue, 0));
    }
    if command == b"s" {
        return Some((Action::Step, 0));
    }
    if command == b"k" {
        return Some((Action::Kill, 0));
    }
    if command == b"qSupported" {
        return reply_bytes(reply, b"PacketSize=0200;swbreak+;hwbreak-");
    }
    if let Some(rest) = command.strip_prefix(b"m") {
        return dispatch_memory(rest, memory_base, memory, reply);
    }
    reply_bytes(reply, b"")
}

fn dispatch_memory(
    spec: &[u8],
    memory_base: u64,
    memory: &[u8],
    reply: &mut [u8],
) -> Option<(Action, usize)> {
    let comma = spec.iter().position(|&byte| byte == b',')?;
    let address = parse_hex(&spec[..comma])?;
    let length = parse_hex_usize(&spec[comma + 1..])?;
    if length > MAX_MEMORY_READ
        || address < memory_base
        || address.checked_sub(memory_base)? > memory.len() as u64
    {
        return reply_bytes(reply, b"E01");
    }
    let offset = usize::try_from(address - memory_base).ok()?;
    let end = offset.checked_add(length)?;
    let bytes = memory.get(offset..end)?;
    if reply.len() < length * 2 {
        return None;
    }
    for (index, &byte) in bytes.iter().enumerate() {
        reply[index * 2] = hex(byte >> 4);
        reply[index * 2 + 1] = hex(byte & 0x0f);
    }
    Some((Action::Reply, length * 2))
}

fn reply_bytes(reply: &mut [u8], bytes: &[u8]) -> Option<(Action, usize)> {
    if reply.len() < bytes.len() {
        return None;
    }
    reply[..bytes.len()].copy_from_slice(bytes);
    Some((Action::Reply, bytes.len()))
}

fn parse_hex(bytes: &[u8]) -> Option<u64> {
    if bytes.is_empty() {
        return None;
    }
    bytes.iter().try_fold(0u64, |value, &byte| {
        value
            .checked_mul(16)?
            .checked_add(u64::from(from_hex(byte)?))
    })
}

fn parse_hex_usize(bytes: &[u8]) -> Option<usize> {
    usize::try_from(parse_hex(bytes)?).ok()
}

fn hex(value: u8) -> u8 {
    match value {
        0..=9 => b'0' + value,
        _ => b'a' + value - 10,
    }
}

fn from_hex(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

/// Boot-time self-test for the pure command boundary.
#[cfg(any(test, feature = "stage32-10-test"))]
pub fn self_test() -> bool {
    let mut context = RegisterContext::ZERO;
    context.values[0] = 0x1122_3344_5566_7788;
    context.values[23] = 0xaabb_ccdd_eeff_0011;
    let mut reply = [0u8; 512];
    if dispatch(b"?", &context, 0x1000, b"", &mut reply) != Some((Action::Reply, 3))
        || &reply[..3] != b"S05"
    {
        return false;
    }
    let Some((Action::Reply, register_length)) = dispatch(b"g", &context, 0x1000, b"", &mut reply)
    else {
        return false;
    };
    if register_length != REGISTER_COUNT * 16
        || &reply[..16] != b"8877665544332211"
        || &reply[23 * 16..24 * 16] != b"1100ffeeddccbbaa"
    {
        return false;
    }
    if dispatch(
        b"m1001,2",
        &context,
        0x1000,
        &[0x01, 0xa2, 0xff],
        &mut reply,
    ) != Some((Action::Reply, 4))
        || &reply[..4] != b"a2ff"
    {
        return false;
    }
    if dispatch(b"m1000,257", &context, 0x1000, &[0; 257], &mut reply) != Some((Action::Reply, 3))
        || &reply[..3] != b"E01"
    {
        return false;
    }
    if dispatch(b"c", &context, 0, b"", &mut reply) != Some((Action::Continue, 0))
        || dispatch(b"s", &context, 0, b"", &mut reply) != Some((Action::Step, 0))
        || dispatch(b"k", &context, 0, b"", &mut reply) != Some((Action::Kill, 0))
    {
        return false;
    }
    let Some((Action::Reply, query_length)) = dispatch(b"qSupported", &context, 0, b"", &mut reply)
    else {
        return false;
    };
    if core::str::from_utf8(&reply[..query_length]).ok()
        != Some("PacketSize=0200;swbreak+;hwbreak-")
    {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::{dispatch, Action, RegisterContext};

    #[test]
    fn rejects_unbounded_memory() {
        let mut reply = [0u8; 8];
        assert_eq!(
            dispatch(b"m0,101", &RegisterContext::ZERO, 0, &[0; 1], &mut reply),
            Some((Action::Reply, 3))
        );
        assert_eq!(&reply[..3], b"E01");
    }
}
