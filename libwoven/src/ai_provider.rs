//! Bounded provider-plugin ABI for WovenAI Stage 23.3.
//!
//! A provider is a separate Ring-3 process. Credentials are deliberately not
//! fields in these frames: a provider owns its WovenVault handle and returns
//! only a bounded result. Network/TLS transport is a provider concern and is
//! not implied by this ABI.

pub const SERVICE_NAME: &[u8] = b"woven.ai.cloud";
pub const PROVIDER_ABI: u64 = 0x0001_0000;
pub const CLOUD_PROVIDER_FLAG: u64 = 1 << 40;
pub const CLOUD_RESULT_FLAG: u64 = 1 << 41;
pub const STATUS_OK: u16 = 0;
pub const STATUS_NO_CREDENTIAL: u16 = 3;
pub const STATUS_UNAVAILABLE: u16 = 4;

pub const fn descriptor() -> u64 {
    PROVIDER_ABI | CLOUD_PROVIDER_FLAG
}

pub const fn result(value: u64) -> u64 {
    value | CLOUD_RESULT_FLAG
}

#[cfg(test)]
mod tests {
    use super::{descriptor, result, CLOUD_PROVIDER_FLAG, CLOUD_RESULT_FLAG, PROVIDER_ABI};

    #[test]
    fn descriptor_identifies_the_cloud_provider_contract() {
        assert_eq!(descriptor(), PROVIDER_ABI | CLOUD_PROVIDER_FLAG);
    }

    #[test]
    fn result_marks_provider_output_without_changing_payload() {
        assert_eq!(result(0x1234), CLOUD_RESULT_FLAG | 0x1234);
    }
}

