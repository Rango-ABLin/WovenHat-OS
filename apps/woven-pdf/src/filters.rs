//! Allocation-free PDF stream filter primitives.
//!
//! Supported here: ASCIIHexDecode, RunLengthDecode and FlateDecode.
//! ASCII85Decode, predictors, LZW and image-specific filters remain TODO.
//! Callers must identify the filter from the parsed stream dictionary and
//! provide a capacity-bounded output buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterError {
    InvalidByte,
    MissingTerminator,
    Truncated,
    OutputFull,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    Identity,
    AsciiHex,
    RunLength,
    Flate,
    Unsupported,
}

fn hex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}
fn whitespace(c: u8) -> bool {
    matches!(c, 0 | 9 | 10 | 12 | 13 | 32)
}

/// Decode ASCIIHex according to PDF's odd-nibble padding rule.
/// The mandatory '>' terminator prevents silent partial outputs.
pub fn ascii_hex(input: &[u8], output: &mut [u8]) -> Result<usize, FilterError> {
    let mut used = 0;
    let mut half = None;
    let mut terminated = false;
    let mut i = 0;
    while i < input.len() {
        let c = input[i];
        i += 1;
        if c == b'>' {
            terminated = true;
            break;
        }
        if whitespace(c) {
            continue;
        }
        let digit = hex(c).ok_or(FilterError::InvalidByte)?;
        if let Some(high) = half.take() {
            if used >= output.len() {
                return Err(FilterError::OutputFull);
            }
            output[used] = (high << 4) | digit;
            used += 1;
        } else {
            half = Some(digit);
        }
    }
    if !terminated {
        return Err(FilterError::MissingTerminator);
    }
    if input[i..].iter().any(|b| !whitespace(*b)) {
        return Err(FilterError::InvalidByte);
    }
    if let Some(high) = half {
        if used >= output.len() {
            return Err(FilterError::OutputFull);
        }
        output[used] = high << 4;
        used += 1;
    }
    Ok(used)
}

/// PDF RunLengthDecode: 0..127 copies n+1 bytes; 129..255 repeats the
/// following byte 257-n times; 128 terminates. Input and output stay bounded.
pub fn run_length(input: &[u8], output: &mut [u8]) -> Result<usize, FilterError> {
    let mut read = 0;
    let mut written = 0;
    loop {
        let control = *input.get(read).ok_or(FilterError::MissingTerminator)?;
        read += 1;
        if control == 128 {
            if read != input.len() {
                return Err(FilterError::InvalidByte);
            }
            return Ok(written);
        }
        let length = if control < 128 {
            control as usize + 1
        } else {
            257 - control as usize
        };
        let end = written.checked_add(length).ok_or(FilterError::OutputFull)?;
        if end > output.len() {
            return Err(FilterError::OutputFull);
        }
        if control < 128 {
            let last = read.checked_add(length).ok_or(FilterError::Truncated)?;
            let bytes = input.get(read..last).ok_or(FilterError::Truncated)?;
            output[written..end].copy_from_slice(bytes);
            read = last;
        } else {
            let value = *input.get(read).ok_or(FilterError::Truncated)?;
            read += 1;
            output[written..end].fill(value);
        }
        written = end;
    }
}

pub fn decode(filter: Filter, input: &[u8], output: &mut [u8]) -> Result<usize, FilterError> {
    match filter {
        Filter::Identity => {
            if input.len() > output.len() {
                return Err(FilterError::OutputFull);
            }
            output[..input.len()].copy_from_slice(input);
            Ok(input.len())
        }
        Filter::AsciiHex => ascii_hex(input, output),
        Filter::RunLength => run_length(input, output),
        Filter::Flate => miniz_oxide::inflate::decompress_slice_iter_to_slice(
            output,
            core::iter::once(input),
            true,
            false,
        )
        .map_err(|_| FilterError::InvalidByte),
        Filter::Unsupported => Err(FilterError::Unsupported),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hex_even_odd_and_whitespace() {
        let mut out = [0; 10];
        assert_eq!(ascii_hex(b"48 65 6C6C6F>", &mut out), Ok(5));
        assert_eq!(&out[..5], b"Hello");
        assert_eq!(ascii_hex(b"4>", &mut out), Ok(1));
        assert_eq!(out[0], 0x40);
    }
    #[test]
    fn hex_rejects_invalid_and_short_buffers() {
        let mut out = [0; 0];
        assert_eq!(ascii_hex(b"4F>", &mut out), Err(FilterError::OutputFull));
        assert_eq!(
            ascii_hex(b"4X>", &mut [0; 4]),
            Err(FilterError::InvalidByte)
        );
        assert_eq!(
            ascii_hex(b"41", &mut [0; 4]),
            Err(FilterError::MissingTerminator)
        );
    }
    #[test]
    fn run_length_literal_and_repeat() {
        let mut out = [0; 10];
        assert_eq!(
            run_length(&[2, b'A', b'B', b'C', 254, b'!', 128], &mut out),
            Ok(6)
        );
        assert_eq!(&out[..6], b"ABC!!!");
    }
    #[test]
    fn flate_zlib_checksum_and_bounds() {
        const ZLIB: &[u8] = &[
            120, 156, 243, 72, 205, 201, 201, 215, 81, 168, 202, 201, 76, 82, 4, 0, 27, 101, 4, 19,
        ];
        let mut out = [0u8; 64];
        assert_eq!(decode(Filter::Flate, ZLIB, &mut out), Ok(12));
        assert_eq!(&out[..12], b"Hello, zlib!");
        assert!(decode(Filter::Flate, ZLIB, &mut [0u8; 2]).is_err());
        let mut bad = ZLIB.to_vec();
        let last = bad.len() - 1;
        bad[last] ^= 1;
        assert!(decode(Filter::Flate, &bad, &mut out).is_err());
    }
    #[test]
    fn run_length_malformed_rejected() {
        assert_eq!(run_length(&[0], &mut [0; 5]), Err(FilterError::Truncated));
        assert_eq!(
            run_length(&[129, b'A'], &mut [0; 5]),
            Err(FilterError::OutputFull)
        );
        assert_eq!(
            run_length(&[128, 0], &mut [0; 5]),
            Err(FilterError::InvalidByte)
        );
        assert_eq!(
            run_length(&[], &mut [0; 5]),
            Err(FilterError::MissingTerminator)
        );
    }
}
