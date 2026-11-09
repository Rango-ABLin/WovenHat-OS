//! Bounded boot policy parsing shared with actual-source host tests.
//!
//! Existing boot.json fields remain valid with an omitted version. New
//! writers may explicitly select version 1; unknown policy is rejected.

use bootloader_api::info::BootPolicy;
use bootloader_boot_config::{BootConfig, LevelFilter};
use serde::Deserialize;

pub const VERSION: u16 = 1;
pub const MAX_CONFIG_BYTES: usize = 4096;
pub const MAX_TIMEOUT_MS: u32 = 30_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    TooLarge,
    Malformed,
    UnsupportedVersion,
    TrailingData,
    UnsupportedPolicy,
}

pub struct Policy {
    pub boot: BootConfig,
    pub handoff: BootPolicy,
    pub kernel_path: Option<heapless::String<127>>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Entry {
    #[default]
    Default,
    Recovery,
    Diagnostics,
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Root {
    #[default]
    Default,
    Recovery,
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
    boot_entry: Entry,
    root: Root,
    timeout_ms: u32,
    command_line: Option<heapless::String<1536>>,
    kernel_path: Option<heapless::String<762>>,
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
            boot_entry: Entry::Default,
            root: Root::Default,
            timeout_ms: 0,
            command_line: None,
            kernel_path: None,
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

pub fn parse(bytes: &[u8]) -> Result<Policy, Error> {
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
    if wire.timeout_ms > MAX_TIMEOUT_MS {
        return Err(Error::UnsupportedPolicy);
    }
    let kernel_path = wire
        .kernel_path
        .as_deref()
        .map(decode_string::<127>)
        .transpose()?;
    let command_line = wire
        .command_line
        .as_deref()
        .map(decode_string::<256>)
        .transpose()?;
    if kernel_path.as_ref().is_some_and(|path| {
        path.is_empty()
            || !path
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-/".contains(&b))
            || path.split('/').any(|part| matches!(part, "" | "." | ".."))
    }) || command_line
        .as_ref()
        .is_some_and(|line| line.chars().any(char::is_control))
    {
        return Err(Error::UnsupportedPolicy);
    }
    let mut result = BootConfig::default();
    result.frame_buffer.minimum_framebuffer_height = wire.frame_buffer.minimum_framebuffer_height;
    result.frame_buffer.minimum_framebuffer_width = wire.frame_buffer.minimum_framebuffer_width;
    result.log_level = wire.log_level;
    result.frame_buffer_logging = wire.frame_buffer_logging;
    result.serial_logging = wire.serial_logging;
    result._test_sentinel = wire._test_sentinel;
    let mut handoff = BootPolicy {
        version: VERSION,
        timeout_ms: wire.timeout_ms,
        ..BootPolicy::default()
    };
    handoff.entry = match wire.boot_entry {
        Entry::Default => 0,
        Entry::Recovery => 1,
        Entry::Diagnostics => 2,
    };
    handoff.root = match wire.root {
        Root::Default => 0,
        Root::Recovery => 1,
    };
    if let Some(command_line) = command_line {
        handoff.command_line_len =
            u16::try_from(command_line.len()).map_err(|_| Error::UnsupportedPolicy)?;
        handoff.command_line[..command_line.len()].copy_from_slice(command_line.as_bytes());
    }
    Ok(Policy {
        boot: result,
        handoff,
        kernel_path,
    })
}

/// serde-json-core 0.5 borrows raw string contents without unescaping them.
/// Decode once into bounded owned UTF-8 before applying path/data validation.
fn decode_string<const N: usize>(raw: &str) -> Result<heapless::String<N>, Error> {
    fn hex4(chars: &mut core::str::Chars<'_>) -> Result<u32, Error> {
        let mut value = 0;
        for _ in 0..4 {
            value = value * 16
                + chars
                    .next()
                    .and_then(|c| c.to_digit(16))
                    .ok_or(Error::Malformed)?;
        }
        Ok(value)
    }
    let mut result = heapless::String::new();
    let mut chars = raw.chars();
    while let Some(mut c) = chars.next() {
        if c == '\\' {
            c = match chars.next().ok_or(Error::Malformed)? {
                '"' => '"',
                '\\' => '\\',
                '/' => '/',
                'b' => '\x08',
                'f' => '\x0c',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                'u' => {
                    let mut scalar = hex4(&mut chars)?;
                    if (0xd800..=0xdbff).contains(&scalar) {
                        if chars.next() != Some('\\') || chars.next() != Some('u') {
                            return Err(Error::Malformed);
                        }
                        let low = hex4(&mut chars)?;
                        if !(0xdc00..=0xdfff).contains(&low) {
                            return Err(Error::Malformed);
                        }
                        scalar = 0x10000 + ((scalar - 0xd800) << 10) + low - 0xdc00;
                    }
                    char::from_u32(scalar).ok_or(Error::Malformed)?
                }
                _ => return Err(Error::Malformed),
            };
        } else if c.is_control() {
            return Err(Error::Malformed);
        }
        result.push(c).map_err(|_| Error::UnsupportedPolicy)?;
    }
    Ok(result)
}
