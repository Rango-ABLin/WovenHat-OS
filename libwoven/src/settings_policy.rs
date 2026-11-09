//! Bounded mutation helpers for the installed settings envelope.

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

#[cfg(test)]
mod tests {
    use super::patch_high_contrast;

    #[test]
    fn patch_is_bounded_and_fixed_width() {
        let mut data = *br#"{"high_contrast":false,"reduced_motion":false}"#;
        assert!(patch_high_contrast(&mut data, true));
        assert!(core::str::from_utf8(&data)
            .unwrap()
            .contains(r#""high_contrast":true "#));
        assert!(patch_high_contrast(&mut data, false));
        assert!(core::str::from_utf8(&data)
            .unwrap()
            .contains(r#""high_contrast":false"#));
        let mut malformed = *br#"{"schema":"wrong"}"#;
        assert!(!patch_high_contrast(&mut malformed, true));
    }
}
