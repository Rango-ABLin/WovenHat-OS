#![cfg_attr(not(test), no_std)]
//! A deliberately narrow, allocation-free PDF text-preview engine.
//! This is NOT a general PDF renderer: no compression, encryption, fonts,
//! page-tree traversal, object streams, or images are supported yet.

pub mod lexer;
pub mod objects;
pub mod xref;

pub const MAX_DOCUMENT_BYTES: usize = 512 * 1024;
pub const MAX_TEXT_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    InvalidHeader,
    UnsupportedVersion,
    TooLarge,
    MissingEof,
    NoText,
    OutputFull,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command<'a> {
    Help,
    Info(&'a [u8]),
    Text(&'a [u8]),
    Invalid,
}

pub fn parse_command<'a>(args: &'a [&'a [u8]]) -> Command<'a> {
    match args {
        [] | [b"--help"] => Command::Help,
        [b"--info", path] if !path.is_empty() && path.len() < 256 => Command::Info(path),
        [b"--text", path] if !path.is_empty() && path.len() < 256 => Command::Text(path),
        _ => Command::Invalid,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocumentInfo {
    pub major: u8,
    pub minor: u8,
    pub byte_length: usize,
}

/// Check a full bounded byte slice. EOF may be followed by PDF whitespace.
pub fn inspect(data: &[u8]) -> Result<DocumentInfo, Error> {
    if data.len() > MAX_DOCUMENT_BYTES {
        return Err(Error::TooLarge);
    }
    if data.len() < 12 || data.get(..5) != Some(b"%PDF-") {
        return Err(Error::InvalidHeader);
    }
    let major = data[5];
    let minor = data[7];
    if !major.is_ascii_digit() || data[6] != b'.' || !minor.is_ascii_digit() {
        return Err(Error::InvalidHeader);
    }
    if major != b'1' && major != b'2' {
        return Err(Error::UnsupportedVersion);
    }
    if !data.windows(5).any(|w| w == b"%%EOF") {
        return Err(Error::MissingEof);
    }
    Ok(DocumentInfo {
        major: major - b'0',
        minor: minor - b'0',
        byte_length: data.len(),
    })
}

fn white(c: u8) -> bool {
    matches!(c, 0 | 9 | 10 | 12 | 13 | 32)
}
fn delimiter(c: u8) -> bool {
    white(c) || matches!(c, b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%')
}
fn token_at(data: &[u8], pos: usize, token: &[u8]) -> bool {
    let end = pos.saturating_add(token.len());
    data.get(pos..end) == Some(token)
        && (pos == 0 || delimiter(data[pos - 1]))
        && (end == data.len() || delimiter(data[end]))
}

/// Extract literal strings directly followed by Tj from unfiltered page
/// content streams. Never interpret arbitrary bytes outside a stream as text.
/// This subset deliberately does not implement PDF text positioning, fonts,
/// ToUnicode maps, arrays/TJ, graphics, or decompression.
pub fn extract_plain_text(data: &[u8], out: &mut [u8]) -> Result<usize, Error> {
    inspect(data)?;
    let mut pos = 0;
    let mut used = 0;
    while pos + 6 <= data.len() {
        if !token_at(data, pos, b"stream") {
            pos += 1;
            continue;
        }
        let after = pos + 6;
        let start = if data.get(after..after + 2) == Some(b"\r\n") {
            after + 2
        } else if data.get(after) == Some(&b'\n') || data.get(after) == Some(&b'\r') {
            after + 1
        } else {
            pos += 6;
            continue;
        };
        let Some(rel) = data[start..].windows(9).position(|w| w == b"endstream") else {
            break;
        };
        let end = start + rel;
        // This reader only supports explicitly unfiltered streams.
        // Checking the containing object's dictionary is required before
        // extracting text, otherwise deflate bytes could produce false output.
        let dict_start = data[..pos].windows(2).rposition(|w| w == b"<<");
        let dict_end = data[..pos].windows(2).rposition(|w| w == b">>");
        let is_unfiltered = match (dict_start, dict_end) {
            (Some(s), Some(e)) if s < e => {
                let dict = &data[s..e];
                !dict.windows(7).any(|w| w == b"/Filter")
                    && !dict.windows(4).any(|w| w == b"/F  ")
            }
            _ => false,
        };
        if is_unfiltered {
            let mut i = start;
            while i < end {
                if data[i] != b'(' {
                    i += 1;
                    continue;
                }
                let mut scratch = [0u8; 4096];
                let mut n = 0;
                let mut level = 1usize;
                let mut j = i + 1;
                while j < end && level > 0 {
                    let c = data[j];
                    if c == b'\\' {
                        j += 1;
                        if j >= end { break; }
                        let mapped = match data[j] {
                            b'n' => b'\n',
                            b'r' => b'\r',
                            b't' => b'\t',
                            b'b' => 8,
                            b'f' => 12,
                            x => x,
                        };
                        if n < scratch.len() { scratch[n] = mapped; n += 1; }
                    } else if c == b'(' {
                        level += 1;
                        if n < scratch.len() { scratch[n] = c; n += 1; }
                    } else if c == b')' {
                        level -= 1;
                        if level > 0 && n < scratch.len() { scratch[n] = c; n += 1; }
                    } else if n < scratch.len() {
                        scratch[n] = c;
                        n += 1;
                    }
                    j += 1;
                }
                if level == 0 {
                    while j < end && white(data[j]) { j += 1; }
                    if token_at(&data[start..end], j - start, b"Tj") && n > 0 {
                        if used + n + 1 > out.len() { return Err(Error::OutputFull); }
                        out[used..used + n].copy_from_slice(&scratch[..n]);
                        used += n;
                        out[used] = b'\n';
                        used += 1;
                    }
                }
                i = j.max(i + 1);
            }
        }
        pos = end + 9;
    }
    if used == 0 { Err(Error::NoText) } else { Ok(used) }
}

#[cfg(test)]
mod tests {
    use super::*;
    const PLAIN: &[u8] = b"%PDF-1.4\n1 0 obj\n<< /Length 20 >>\nstream\nBT (Hello WovenPDF) Tj ET\nendstream\nendobj\n%%EOF\n";
    #[test]
    fn recognizes_document() {
        let d = inspect(PLAIN).unwrap();
        assert_eq!((d.major, d.minor), (1, 4));
        assert_eq!(d.byte_length, PLAIN.len());
    }
    #[test]
    fn extracts_uncompressed_literal_tj() {
        let mut out = [0; 100];
        let n = extract_plain_text(PLAIN, &mut out).unwrap();
        assert_eq!(&out[..n], b"Hello WovenPDF\n");
    }
    #[test]
    fn rejects_invalid_and_truncated() {
        assert_eq!(inspect(b"not a pdf"), Err(Error::InvalidHeader));
        assert_eq!(inspect(b"%PDF-1.4\nbody"), Err(Error::MissingEof));
    }
    #[test]
    fn refuses_filtered_streams() {
        let source = b"%PDF-1.4\n1 0 obj << /Filter /FlateDecode >>\nstream\nBT (wrong) Tj ET\nendstream\n%%EOF";
        let mut out = [0; 100];
        assert_eq!(extract_plain_text(source, &mut out), Err(Error::NoText));
    }
    #[test]
    fn output_capacity_is_checked() {
        let mut out = [0; 2];
        assert_eq!(extract_plain_text(PLAIN, &mut out), Err(Error::OutputFull));
    }
    #[test]
    fn commands_are_bounded() {
        assert_eq!(parse_command(&[b"--text", b"/mnt/a.pdf"]), Command::Text(b"/mnt/a.pdf"));
        assert_eq!(parse_command(&[b"--text", &[b'x'; 256]]), Command::Invalid);
    }
}
