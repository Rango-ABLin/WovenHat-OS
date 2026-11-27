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
    let Some(start) = find_single_key(data) else {
        return false;
    };
    // Reject duplicate keys rather than silently updating an ambiguous record.
    if data[start + KEY.len()..].windows(KEY.len()).any(|window| window == KEY) {
        return false;
    }
    let value_start = start + KEY.len();
    let value: &[u8] = if enabled { b"true " } else { b"false" };
    let Some(destination) = data.get_mut(value_start..value_start + value.len()) else {
        return false;
    };
    if destination != b"false" && destination != b"true " {
        return false;
    }
    destination.copy_from_slice(value);
    true
}

/// Locate the sole `high_contrast` key, refusing ambiguous records with a
/// duplicate key anywhere after the first match. This is deliberately shared
/// by the writer and reader so an installed record is never patched or shown
/// as High Contrast from an ambiguous value.
fn find_single_key(data: &[u8]) -> Option<usize> {
    const KEY: &[u8] = b"\"high_contrast\":";
    let start = data.windows(KEY.len()).position(|window| window == KEY)?;
    if data[start + KEY.len()..]
        .windows(KEY.len())
        .any(|window| window == KEY)
    {
        return None;
    }
    Some(start)
}

/// Read the effective high-contrast flag from a bounded v2 record. Returns
/// `None` for absent, malformed or ambiguous values, so callers fail closed.
pub fn read_high_contrast(data: &[u8]) -> Option<bool> {
    const KEY: &[u8] = b"\"high_contrast\":";
    let start = find_single_key(data)?;
    let value = data.get(start + KEY.len()..start + KEY.len() + 5)?;
    if value == b"true " {
        Some(true)
    } else if value == b"false" {
        Some(false)
    } else {
        None
    }
}

pub fn patch_reduced_motion(data: &mut [u8], enabled: bool) -> bool {
    patch_fixed_boolean(data, b"reduced_motion", enabled)
}

pub fn read_reduced_motion(data: &[u8]) -> Option<bool> {
    read_boolean(data, b"reduced_motion")
}

/// Themes use fixed-width values (`dark ` and `light`) so an installed record
/// can be updated in place without changing its offsets.
pub fn patch_theme(data: &mut [u8], light: bool) -> bool {
    patch_fixed_string(data, b"theme", if light { b"light" } else { b"dark " })
}

pub fn read_light_theme(data: &[u8]) -> Option<bool> {
    let value = string_field(data, b"theme")?;
    match value.trim_ascii_end() {
        "dark" => Some(false),
        "light" => Some(true),
        _ => None,
    }
}

fn patch_fixed_boolean(data: &mut [u8], name: &[u8], enabled: bool) -> bool {
    let Some(start) = find_single_named_key(data, name) else {
        return false;
    };
    let value_start = start + name.len() + 3;
    let value: &[u8] = if enabled { b"true " } else { b"false" };
    let Some(destination) = data.get_mut(value_start..value_start + value.len()) else {
        return false;
    };
    if destination != b"false" && destination != b"true " {
        return false;
    }
    destination.copy_from_slice(value);
    true
}

fn patch_fixed_string(data: &mut [u8], name: &[u8], value: &[u8]) -> bool {
    let Some(start) = find_single_named_key(data, name) else {
        return false;
    };
    let value_start = start + name.len() + 4;
    let Some(destination) = data.get_mut(value_start..value_start + value.len()) else {
        return false;
    };
    if &*destination != b"dark " && &*destination != b"light" {
        return false;
    }
    destination.copy_from_slice(value);
    true
}

fn find_single_named_key(data: &[u8], name: &[u8]) -> Option<usize> {
    let mut key = [0u8; 32];
    let len = name.len().checked_add(2)?;
    if len > key.len() {
        return None;
    }
    key[0] = b'"';
    key[1..1 + name.len()].copy_from_slice(name);
    key[1 + name.len()] = b'"';
    let start = data.windows(len).position(|window| window == &key[..len])?;
    if data[start + len..]
        .windows(len)
        .any(|window| window == &key[..len])
    {
        return None;
    }
    Some(start)
}

fn read_boolean(data: &[u8], name: &[u8]) -> Option<bool> {
    let start = find_single_named_key(data, name)?;
    let value = data.get(start + name.len() + 3..start + name.len() + 8)?;
    if value == b"true " {
        Some(true)
    } else if value == b"false" {
        Some(false)
    } else {
        None
    }
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
    if !changed && string_field(data, b"theme").is_some() {
        output[..data.len()].copy_from_slice(data);
        return Ok(Migration {
            length: data.len(),
            changed: false,
        });
    }
    if !changed {
        let end = data
            .iter()
            .rposition(|byte| *byte == b'}')
            .ok_or(MigrationError::Malformed)?;
        if end + 22 > output.len() {
            return Err(MigrationError::OutputTooSmall);
        }
        output[..end].copy_from_slice(&data[..end]);
        let mut writer = Writer::new(output);
        writer.position = end;
        writer.bytes(br#","theme":"dark "}"#)?;
        writer.bytes(b"\n")?;
        return Ok(Migration {
            length: writer.position,
            changed: true,
        });
    }

    let user = bounded_string_field(data, b"user", "Ada", 32)?;
    let timezone = bounded_string_field(data, b"timezone", "UTC", 32)?;
    let keyboard = bounded_string_field(data, b"keyboard", "us", 16)?;
    let locale = bounded_string_field(data, b"locale", "en-US", 32)?;
    let network = bounded_string_field(data, b"network", "automatic", 32)?;
    let taskbar_alignment = bounded_string_field(data, b"taskbar_alignment", "center", 16)?;
    let taskbar_icon_size = bounded_string_field(data, b"taskbar_icon_size", "medium", 16)?;
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
    writer.bytes(br#","theme":""#)?;
    writer.string("dark ")?;
    writer.bytes(b"\"")?;
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
        Self {
            output,
            position: 0,
        }
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
        CURRENT_SCHEMA, MAX_SETTINGS_BYTES, MigrationError, migrate, patch_high_contrast,
        patch_reduced_motion, patch_theme, read_high_contrast, read_light_theme,
        read_reduced_motion,
    };

    #[test]
    fn ambiguous_and_malformed_values_fail_closed_without_mutation() {
        let duplicate = *br#"{"high_contrast":false,"high_contrast":true }"#;
        let mut data = duplicate;
        assert!(!patch_high_contrast(&mut data, true));
        assert_eq!(data, duplicate);
        assert_eq!(read_high_contrast(&duplicate), None);

        let nested = *br#"{"high_contrast":false,"nested":{"high_contrast":false}}"#;
        let mut data = nested;
        assert!(!patch_high_contrast(&mut data, true));
        assert_eq!(data, nested);
        assert_eq!(read_high_contrast(&nested), None);

        for bad in [
            &br#"{"high_contrast":null }"#[..],
            &br#"{"high_contrast":truex}"#[..],
        ] {
            let mut data = [0u8; MAX_SETTINGS_BYTES];
            data[..bad.len()].copy_from_slice(bad);
            assert!(!patch_high_contrast(&mut data[..bad.len()], true));
            assert_eq!(read_high_contrast(bad), None);
        }
    }

    #[test]
    fn malformed_and_duplicate_values_are_rejected_without_mutation() {
        for original in [
            br#"{"high_contrast":null }"#.as_slice(),
            br#"{"high_contrast":truex}"#.as_slice(),
            br#"{"high_contrast":false,"high_contrast":false}"#.as_slice(),
            br#"{"high_contrast":false,"nested":{"high_contrast":false}}"#.as_slice(),
        ] {
            let mut data = original.to_vec();
            assert!(!patch_high_contrast(&mut data, true));
            assert_eq!(data, original);
        }
    }

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
        assert!(
            !migrate(&output[..migration.length], &mut [0u8; MAX_SETTINGS_BYTES])
                .unwrap()
                .changed
        );
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
        let input = br#"{"schema":"wovenhat-installed-v2","user":"Ada","theme":"dark "}"#;
        let mut output = [0u8; MAX_SETTINGS_BYTES];
        let migration = migrate(input, &mut output).unwrap();
        assert!(!migration.changed);
        assert_eq!(&output[..migration.length], input);
        assert_eq!(CURRENT_SCHEMA, b"wovenhat-installed-v2");
    }

    #[test]
    fn patches_personalization_fields_in_place() {
        let mut data = *br#"{"reduced_motion":false,"theme":"dark "}"#;
        assert!(patch_reduced_motion(&mut data, true));
        assert!(patch_theme(&mut data, true));
        assert_eq!(read_reduced_motion(&data), Some(true));
        assert_eq!(read_light_theme(&data), Some(true));
        assert!(patch_reduced_motion(&mut data, false));
        assert!(patch_theme(&mut data, false));
        assert_eq!(read_reduced_motion(&data), Some(false));
        assert_eq!(read_light_theme(&data), Some(false));
    }
}
