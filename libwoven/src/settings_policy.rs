//! Bounded mutation helpers for the installed settings envelope.

pub const CURRENT_SCHEMA: &[u8] = b"wovenhat-installed-v2";
pub const LEGACY_SCHEMA: &[u8] = b"wovenhat-installed-v1";
pub const MAX_SETTINGS_BYTES: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MigrationError {
    Malformed,
    UnsupportedSchema,
    OutputTooSmall,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Migration {
    pub length: usize,
    pub changed: bool,
}

/// Replace the fixed-width `high_contrast` value without parsing or
/// reallocating arbitrary JSON. The true form has one trailing space so both
/// boolean encodings occupy five bytes and an existing file never needs
/// truncation.
pub fn patch_high_contrast(data: &mut [u8], enabled: bool) -> bool {
    const KEY: &[u8] = b"\"high_contrast\":";
    let Some(start) = data.windows(KEY.len()).position(|window| window == KEY) else {
        return false;
    };
    let value_start = start + KEY.len();
    let value: &[u8] = if enabled { b"true " } else { b"false" };
    let Some(destination) = data.get_mut(value_start..value_start + value.len()) else {
        return false;
    };
    if enabled {
        if !destination.starts_with(b"false") && !destination.starts_with(b"true ") {
            return false;
        }
    } else if !destination.starts_with(b"false") && !destination.starts_with(b"true ") {
        return false;
    }
    destination.copy_from_slice(value);
    true
}

pub fn migrate(data: &[u8], output: &mut [u8]) -> Result<Migration, MigrationError> {
    if data.is_empty() || data.len() > MAX_SETTINGS_BYTES || output.len() < MAX_SETTINGS_BYTES {
        return Err(MigrationError::Malformed);
    }
    let schema = string_field(data, b"schema").ok_or(MigrationError::Malformed)?;
    let changed = match schema.as_bytes() {
        CURRENT_SCHEMA => false,
        LEGACY_SCHEMA => true,
        _ => return Err(MigrationError::UnsupportedSchema),
    };
    if !changed {
        output[..data.len()].copy_from_slice(data);
        return Ok(Migration {
            length: data.len(),
            changed: false,
        });
    }

    let user = bounded_string_field(data, b"user", "Ada", 32)?;
    let timezone = bounded_string_field(data, b"timezone", "UTC", 32)?;
    let keyboard = bounded_string_field(data, b"keyboard", "us", 16)?;
    let locale = bounded_string_field(data, b"locale", "en-US", 32)?;
    let network = bounded_string_field(data, b"network", "automatic", 32)?;
    let taskbar_alignment =
        bounded_string_field(data, b"taskbar_alignment", "center", 16)?;
    let taskbar_icon_size =
        bounded_string_field(data, b"taskbar_icon_size", "medium", 16)?;
    let pinned_apps = bounded_string_field(
        data,
        b"pinned_apps",
        "WovenFiles,WovenTerminal,WovenCalculator",
        128,
    )?;

    let mut writer = Writer::new(output);
    writer.bytes(br#"{"schema":""#)?;
    writer.bytes(CURRENT_SCHEMA)?;
    writer.bytes(br#"","user":""#)?;
    writer.string(user)?;
    writer.bytes(br#"","timezone":""#)?;
    writer.string(timezone)?;
    writer.bytes(br#"","keyboard":""#)?;
    writer.string(keyboard)?;
    writer.bytes(br#"","locale":""#)?;
    writer.string(locale)?;
    writer.bytes(br#"","network":""#)?;
    writer.string(network)?;
    writer.bytes(br#"","high_contrast":"#)?;
    writer.boolean(bool_field(data, b"high_contrast").unwrap_or(false))?;
    writer.bytes(br#","reduced_motion":"#)?;
    writer.boolean(bool_field(data, b"reduced_motion").unwrap_or(false))?;
    writer.bytes(br#","taskbar_alignment":""#)?;
    writer.string(taskbar_alignment)?;
    writer.bytes(br#"","taskbar_icon_size":""#)?;
    writer.string(taskbar_icon_size)?;
    writer.bytes(br#"","pinned_apps":""#)?;
    writer.string(pinned_apps)?;
    writer.bytes(b"\"}\n")?;
    Ok(Migration {
        length: writer.position,
        changed: true,
    })
}

fn bounded_string_field<'a>(
    data: &'a [u8],
    key: &[u8],
    default: &'static str,
    maximum: usize,
) -> Result<&'a str, MigrationError> {
    let value = match string_field(data, key) {
        Some(value) => value,
        None if key.is_empty() => return Err(MigrationError::Malformed),
        None => default,
    };
    if value.len() > maximum || !value.bytes().all(safe_string_byte) {
        return Err(MigrationError::Malformed);
    }
    Ok(value)
}

fn safe_string_byte(byte: u8) -> bool {
    (byte >= 0x20 && byte != b'\\' && byte != b'"') || matches!(byte, b',' | b'.' | b'-' | b'_')
}

fn string_field<'a>(data: &'a [u8], key: &[u8]) -> Option<&'a str> {
    let value = field_value(data, key)?;
    let rest = value.strip_prefix(b"\"")?;
    let end = rest.iter().position(|byte| *byte == b'"')?;
    core::str::from_utf8(&rest[..end]).ok()
}

fn bool_field(data: &[u8], key: &[u8]) -> Option<bool> {
    let value = field_value(data, key)?;
    if value.starts_with(b"true") {
        Some(true)
    } else if value.starts_with(b"false") {
        Some(false)
    } else {
        None
    }
}

fn field_value<'a>(data: &'a [u8], key: &[u8]) -> Option<&'a [u8]> {
    let mut quoted = [0u8; 32];
    let key_len = key.len().checked_add(2)?;
    if key_len > quoted.len() {
        return None;
    }
    quoted[0] = b'"';
    quoted[1..1 + key.len()].copy_from_slice(key);
    quoted[1 + key.len()] = b'"';
    let start = data
        .windows(key_len)
        .position(|window| window == &quoted[..key_len])?
        + key_len;
    let rest = &data[start..];
    let colon = rest.iter().position(|byte| *byte == b':')?;
    Some(rest[colon + 1..].trim_ascii_start())
}

struct Writer<'a> {
    output: &'a mut [u8],
    position: usize,
}

impl<'a> Writer<'a> {
    fn new(output: &'a mut [u8]) -> Self {
        Self { output, position: 0 }
    }

    fn bytes(&mut self, bytes: &[u8]) -> Result<(), MigrationError> {
        let end = self
            .position
            .checked_add(bytes.len())
            .ok_or(MigrationError::OutputTooSmall)?;
        self.output
            .get_mut(self.position..end)
            .ok_or(MigrationError::OutputTooSmall)?
            .copy_from_slice(bytes);
        self.position = end;
        Ok(())
    }

    fn string(&mut self, value: &str) -> Result<(), MigrationError> {
        self.bytes(value.as_bytes())
    }

    fn boolean(&mut self, value: bool) -> Result<(), MigrationError> {
        self.bytes(if value { b"true" } else { b"false" })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        migrate, patch_high_contrast, MigrationError, CURRENT_SCHEMA, MAX_SETTINGS_BYTES,
    };

    #[test]
    fn patch_is_bounded_and_fixed_width() {
        let mut data = *br#"{"high_contrast":false,"reduced_motion":false}"#;
        assert!(patch_high_contrast(&mut data, true));
        assert!(
            core::str::from_utf8(&data)
                .unwrap()
                .contains(r#""high_contrast":true "#)
        );
        assert!(patch_high_contrast(&mut data, false));
        assert!(
            core::str::from_utf8(&data)
                .unwrap()
                .contains(r#""high_contrast":false"#)
        );
        let mut malformed = *br#"{"schema":"wrong"}"#;
        assert!(!patch_high_contrast(&mut malformed, true));
    }

    #[test]
    fn migrates_installed_v1_with_bounded_defaults() {
        let input = br#"{"schema":"wovenhat-installed-v1","user":"Lin","timezone":"UTC","keyboard":"gb","locale":"en-GB","network":"wired","high_contrast":true,"reduced_motion":false}"#;
        let mut output = [0u8; MAX_SETTINGS_BYTES];
        let migration = migrate(input, &mut output).unwrap();
        assert!(migration.changed);
        let text = core::str::from_utf8(&output[..migration.length]).unwrap();
        assert!(text.contains("\"schema\":\"wovenhat-installed-v2\""));
        assert!(text.contains("\"user\":\"Lin\""));
        assert!(text.contains("\"high_contrast\":true"));
        assert!(text.contains("\"taskbar_alignment\":\"center\""));
        assert!(migrate(&output[..migration.length], &mut [0u8; MAX_SETTINGS_BYTES])
            .unwrap()
            .changed == false);
    }

    #[test]
    fn rejects_unsupported_or_unsafe_schema_values() {
        let mut output = [0u8; MAX_SETTINGS_BYTES];
        assert_eq!(
            migrate(br#"{"schema":"future"}"#, &mut output),
            Err(MigrationError::UnsupportedSchema)
        );
        assert_eq!(
            migrate(
                br#"{"schema":"wovenhat-installed-v1","user":"bad\"value"}"#,
                &mut output,
            ),
            Err(MigrationError::Malformed)
        );
        assert_eq!(
            migrate(
                br#"{"schema":"wovenhat-installed-v1","user":"012345678901234567890123456789012"}"#,
                &mut output,
            ),
            Err(MigrationError::Malformed)
        );
    }

    #[test]
    fn current_schema_is_byte_stable() {
        let input = br#"{"schema":"wovenhat-installed-v2","user":"Ada"}"#;
        let mut output = [0u8; MAX_SETTINGS_BYTES];
        let migration = migrate(input, &mut output).unwrap();
        assert!(!migration.changed);
        assert_eq!(&output[..migration.length], input);
        assert_eq!(CURRENT_SCHEMA, b"wovenhat-installed-v2");
    }
}
