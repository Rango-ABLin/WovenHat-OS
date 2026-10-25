//! Strict, bounded classic PDF xref-table reader.
//!
//! Only classic uncompressed cross-reference tables are supported. Xref
//! streams, hybrid references and incremental updates are rejected for now.
//! No heap allocations are required.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XrefError {
    MissingStart,
    InvalidOffset,
    InvalidTable,
    Truncated,
    Capacity,
    Unsupported,
    DuplicateObject,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Entry {
    pub object: u32,
    pub generation: u16,
    pub offset: usize,
    pub in_use: bool,
}

const MAX_ENTRIES: usize = 4096;

fn ws(b: u8) -> bool { matches!(b, 0 | 9 | 10 | 12 | 13 | 32) }

fn skip(input: &[u8], at: &mut usize) {
    while *at < input.len() && ws(input[*at]) { *at += 1; }
}

fn line<'a>(input: &'a [u8], at: &mut usize) -> Result<&'a [u8], XrefError> {
    if *at >= input.len() { return Err(XrefError::Truncated); }
    let start = *at;
    while *at < input.len() && input[*at] != b'\n' && input[*at] != b'\r' {
        *at += 1;
    }
    let end = *at;
    if *at < input.len() {
        let first = input[*at];
        *at += 1;
        if first == b'\r' && input.get(*at) == Some(&b'\n') { *at += 1; }
    }
    Ok(&input[start..end])
}

fn number(raw: &[u8]) -> Result<usize, XrefError> {
    if raw.is_empty() || !raw.iter().all(u8::is_ascii_digit) {
        return Err(XrefError::InvalidTable);
    }
    let mut n = 0usize;
    for &b in raw {
        n = n.checked_mul(10).and_then(|n| n.checked_add((b - b'0') as usize))
            .ok_or(XrefError::InvalidOffset)?;
    }
    Ok(n)
}

fn fields(mut row: &[u8]) -> Result<(usize, usize), XrefError> {
    while row.first().is_some_and(|b| ws(*b)) { row = &row[1..]; }
    let first = row.iter().position(|b| ws(*b)).ok_or(XrefError::InvalidTable)?;
    let start = number(&row[..first])?;
    row = &row[first..];
    while row.first().is_some_and(|b| ws(*b)) { row = &row[1..]; }
    let end = row.iter().position(|b| ws(*b)).unwrap_or(row.len());
    let count = number(&row[..end])?;
    if row[end..].iter().any(|b| !ws(*b)) { return Err(XrefError::InvalidTable); }
    Ok((start, count))
}

/// Resolve `startxref` from the final revision. Trailer parsing and
/// /Prev chains are future work, so documents containing /Prev are rejected.
pub fn startxref(input: &[u8]) -> Result<usize, XrefError> {
    let eof = input.windows(5).rposition(|b| b == b"%%EOF").ok_or(XrefError::MissingStart)?;
    let mark = input[..eof].windows(9).rposition(|b| b == b"startxref")
        .ok_or(XrefError::MissingStart)?;
    let mut i = mark + 9;
    skip(input, &mut i);
    let start = i;
    while input.get(i).is_some_and(u8::is_ascii_digit) { i += 1; }
    let off = number(&input[start..i]).map_err(|_| XrefError::InvalidOffset)?;
    if off >= input.len() || input.get(off..off + 4) != Some(b"xref") {
        return Err(XrefError::Unsupported);
    }
    Ok(off)
}

/// Load a classic xref table with strict line lengths and object bounds.
/// The returned number tells callers how many entries in `out` are valid.
pub fn read_table(input: &[u8], out: &mut [Entry]) -> Result<usize, XrefError> {
    let offset = startxref(input)?;
    let mut cursor = offset + 4;
    let mut used = 0usize;
    loop {
        skip(input, &mut cursor);
        if input.get(cursor..cursor + 7) == Some(b"trailer") {
            // /Prev indicates incremental updates which this routine does not merge.
            let rest = &input[cursor + 7..];
            let end = rest.windows(9).position(|b| b == b"startxref").unwrap_or(rest.len());
            if rest[..end].windows(5).any(|b| b == b"/Prev") {
                return Err(XrefError::Unsupported);
            }
            return Ok(used);
        }
        let header = line(input, &mut cursor)?;
        let (first, count) = fields(header)?;
        if count == 0 || count > MAX_ENTRIES || used.checked_add(count).ok_or(XrefError::Capacity)? > out.len() {
            return Err(XrefError::Capacity);
        }
        if first.checked_add(count).ok_or(XrefError::Capacity)? > MAX_ENTRIES {
            return Err(XrefError::Capacity);
        }
        for index in 0..count {
            let row = line(input, &mut cursor)?;
            // Classic xref entry is exactly ten decimal offset characters,
            // space, five generation characters, space, 'n' or 'f'.
            if row.len() < 18 || !row[..10].iter().all(u8::is_ascii_digit)
                || row[10] != b' ' || !row[11..16].iter().all(u8::is_ascii_digit)
                || row[16] != b' ' || !matches!(row[17], b'n' | b'f')
                || row[18..].iter().any(|b| !ws(*b)) {
                return Err(XrefError::InvalidTable);
            }
            let object = (first + index) as u32;
            if out[..used].iter().any(|entry| entry.object == object) {
                return Err(XrefError::DuplicateObject);
            }
            let offset = number(&row[..10])?;
            let generation = number(&row[11..16])? as u16;
            let in_use = row[17] == b'n';
            if in_use && offset >= input.len() { return Err(XrefError::InvalidOffset); }
            out[used] = Entry { object, generation, offset, in_use };
            used += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document() -> std::vec::Vec<u8> {
        let mut pdf = b"%PDF-1.4\n".to_vec();
        let position = pdf.len();
        pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog >>\nendobj\n");
        let xref = pdf.len();
        pdf.extend_from_slice(format!(
            "xref\n0 2\n0000000000 65535 f \n{position:010} 00000 n \ntrailer\n<< /Size 2 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n"
        ).as_bytes());
        pdf
    }

    #[test]
    fn reads_classic_table() {
        let bytes = document();
        let mut entries = [Entry::default(); 4];
        assert_eq!(read_table(&bytes, &mut entries), Ok(2));
        assert_eq!(entries[1].object, 1);
        assert!(entries[1].in_use);
        assert_eq!(&bytes[entries[1].offset..entries[1].offset + 3], b"1 0");
    }

    #[test]
    fn detects_invalid_pointer_and_capacity() {
        let bytes = document();
        let mut small = [Entry::default(); 1];
        assert_eq!(read_table(&bytes, &mut small), Err(XrefError::Capacity));
        assert_eq!(startxref(b"%PDF-1.4\nstartxref\n500\n%%EOF"), Err(XrefError::Unsupported));
    }

    #[test]
    fn rejects_missing_and_corrupted_table() {
        assert_eq!(startxref(b"%PDF-1.4\n%%EOF"), Err(XrefError::MissingStart));
        let mut bad = document();
        let position = startxref(&bad).unwrap();
        bad[position + 5] = b'X';
        let mut entries = [Entry::default(); 4];
        assert_eq!(read_table(&bad, &mut entries), Err(XrefError::InvalidTable));
    }

    #[test]
    fn rejects_incremental_trailer() {
        let mut pdf = b"%PDF-1.4\nxref\n0 1\n0000000000 65535 f \ntrailer\n<< /Size 1 /Prev 3 >>\nstartxref\n9\n%%EOF".to_vec();
        let mut entries = [Entry::default(); 2];
        assert_eq!(read_table(&pdf, &mut entries), Err(XrefError::Unsupported));
        pdf.clear();
    }
}
