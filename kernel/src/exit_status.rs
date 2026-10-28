//! Successful signed exit payloads for the scalar wait/join ABI.
//! Error sentinels remain the full-width u64::MAX and u64::MAX - 1.

pub const fn encode(status: i32) -> u64 {
    // Zero extension keeps every i32 payload separate from 64-bit errors.
    // A caller recovers the signed status with `raw as i32` after error checks.
    status as u32 as u64
}

#[cfg(test)]
mod tests {
    use super::encode;

    #[test]
    fn signed_payloads_do_not_collide_with_error_sentinels() {
        for status in [-1, -2, i32::MIN, i32::MAX, 0, 37] {
            let raw = encode(status);
            assert_ne!(raw, u64::MAX, "status {status}");
            assert_ne!(raw, u64::MAX - 1, "status {status}");
            assert_eq!(raw, u64::from(status as u32));
            assert_eq!(raw as i32, status);
        }
    }
}
