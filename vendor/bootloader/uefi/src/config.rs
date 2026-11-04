//! Bounded boot policy parsing shared with actual-source host tests.
//!
//! Existing boot.json fields remain valid with an omitted version. New
//! writers may explicitly select version 1; unknown policy is rejected.

use bootloader_boot_config::{BootConfig, LevelFilter};
use serde::Deserialize;

pub const VERSION: u16 = 1;
pub const MAX_CONFIG_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    TooLarge,
    Malformed,
    UnsupportedVersion,
    TrailingData,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FrameBuffer {
    minimum_framebuffer_height: Option<u64>,
    minimum_framebuffer_width: Option<u64>,
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Configuration {
    version: u16,
    frame_buffer: FrameBuffer,
    log_level: LevelFilter,
    frame_buffer_logging: bool,
    serial_logging: bool,
    _test_sentinel: u64,
}

impl Default for Configuration {
    fn default() -> Self {
        let defaults = BootConfig::default();
        Self {
            version: VERSION,
            frame_buffer: FrameBuffer::default(),
            log_level: defaults.log_level,
            frame_buffer_logging: defaults.frame_buffer_logging,
            serial_logging: defaults.serial_logging,
            _test_sentinel: defaults._test_sentinel,
        }
    }
}

pub fn validate_size(size: usize) -> Result<(), Error> {
    if size > MAX_CONFIG_BYTES {
        return Err(Error::TooLarge);
    }
    if size == 0 {
        return Err(Error::Malformed);
    }
    Ok(())
}

pub fn parse(bytes: &[u8]) -> Result<BootConfig, Error> {
    validate_size(bytes.len())?;
    let (wire, consumed): (Configuration, usize) =
        serde_json_core::from_slice(bytes).map_err(|error| match error {
            serde_json_core::de::Error::TrailingCharacters => Error::TrailingData,
            _ => Error::Malformed,
        })?;
    if bytes[consumed..]
        .iter()
        .any(|byte| !matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
    {
        return Err(Error::TrailingData);
    }
    if wire.version != VERSION {
        return Err(Error::UnsupportedVersion);
    }
    let mut result = BootConfig::default();
    result.frame_buffer.minimum_framebuffer_height = wire.frame_buffer.minimum_framebuffer_height;
    result.frame_buffer.minimum_framebuffer_width = wire.frame_buffer.minimum_framebuffer_width;
    result.log_level = wire.log_level;
    result.frame_buffer_logging = wire.frame_buffer_logging;
    result.serial_logging = wire.serial_logging;
    result._test_sentinel = wire._test_sentinel;
    Ok(result)
}
