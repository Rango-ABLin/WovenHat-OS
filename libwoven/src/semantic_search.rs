//! Bounded semantic-search core for WovenAI Stage 23.5.
//!
//! The first OS search boundary is deliberately deterministic and allocation
//! free: normalized ASCII terms are indexed into fixed-size records and query
//! hits are ranked by term coverage, then by stable insertion order. A later
//! provider may replace the scorer, but it must preserve these capacity and
//! ownership limits at the IPC boundary.

pub const MAX_ENTRIES: usize = 32;
pub const MAX_TERMS_PER_ENTRY: usize = 12;
pub const MAX_TERM_BYTES: usize = 16;
pub const MAX_TITLE_BYTES: usize = 48;
pub const MAX_QUERY_BYTES: usize = 128;
pub const MAX_HITS: usize = 8;
const MAGIC: [u8; 4] = *b"WSI1";
const VERSION: u8 = 1;
const ENTRY_ENCODED_LEN: usize = 8 + 8 + 1 + 1 + MAX_TITLE_BYTES;
pub const MAX_ENCODED_BYTES: usize = 8 + MAX_ENTRIES * ENTRY_ENCODED_LEN;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    TooShort,
    BadMagic,
    BadVersion,
    BadLength,
    BadChecksum,
    InvalidEntry,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Application = 1,
    File = 2,
    Setting = 3,
    Message = 4,
    Document = 5,
    Command = 6,
}

impl Kind {
    fn decode(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Application),
            2 => Some(Self::File),
            3 => Some(Self::Setting),
            4 => Some(Self::Message),
            5 => Some(Self::Document),
            6 => Some(Self::Command),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: u64,
    /// Owner 0 is public metadata; nonzero owners are visible only to that
    /// owner when queried through `Index::search_for`.
    pub owner: u64,
    pub kind: Kind,
    pub title: [u8; MAX_TITLE_BYTES],
    pub title_len: u8,
    terms: [[u8; MAX_TERM_BYTES]; MAX_TERMS_PER_ENTRY],
    term_lengths: [u8; MAX_TERMS_PER_ENTRY],
    term_count: u8,
}

impl Entry {
    pub fn new(id: u64, kind: Kind, title: &[u8]) -> Option<Self> {
        Self::new_owned(0, id, kind, title)
    }

    pub fn new_owned(owner: u64, id: u64, kind: Kind, title: &[u8]) -> Option<Self> {
        let mut entry = Self {
            id,
            owner,
            kind,
            title: [0; MAX_TITLE_BYTES],
            title_len: 0,
            terms: [[0; MAX_TERM_BYTES]; MAX_TERMS_PER_ENTRY],
            term_lengths: [0; MAX_TERMS_PER_ENTRY],
            term_count: 0,
        };
        entry.title_len = copy_normalized(title, &mut entry.title)? as u8;
        let mut offset = 0;
        while offset < title.len() {
            while offset < title.len() && !is_term_byte(title[offset]) {
                offset += 1;
            }
            let start = offset;
            while offset < title.len() && is_term_byte(title[offset]) {
                offset += 1;
            }
            if start < offset && entry.term_count < MAX_TERMS_PER_ENTRY as u8 {
                let length = offset - start;
                if length <= MAX_TERM_BYTES {
                    let index = entry.term_count as usize;
                    for (destination, source) in entry.terms[index]
                        .iter_mut()
                        .zip(&title[start..offset])
                    {
                        *destination = source.to_ascii_lowercase();
                    }
                    entry.term_lengths[index] = length as u8;
                    entry.term_count += 1;
                }
            }
        }
        Some(entry)
    }

    fn matches(&self, query: &[u8]) -> u16 {
        let mut score = 0u16;
        for query_term in Terms::new(query) {
            for index in 0..self.term_count as usize {
                if self.term_lengths[index] as usize == query_term.len
                    && self.terms[index][..query_term.len] == query_term.bytes[..query_term.len]
                {
                    score = score.saturating_add(1);
                    break;
                }
            }
        }
        score
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    pub id: u64,
    pub kind: Kind,
    pub score: u16,
}

pub struct Index {
    entries: [Option<Entry>; MAX_ENTRIES],
    length: usize,
}

impl Index {
    pub const fn new() -> Self {
        Self {
            entries: [None; MAX_ENTRIES],
            length: 0,
        }
    }

    pub fn insert(&mut self, entry: Entry) -> bool {
        if self.length == MAX_ENTRIES {
            return false;
        }
        self.entries[self.length] = Some(entry);
        self.length += 1;
        true
    }

    pub fn upsert(&mut self, entry: Entry) -> bool {
        if let Some(position) = self.entries[..self.length]
            .iter()
            .position(|existing| existing.is_some_and(|value| value.id == entry.id && value.owner == entry.owner))
        {
            self.entries[position] = Some(entry);
            true
        } else {
            self.insert(entry)
        }
    }

    pub fn remove(&mut self, owner: u64, id: u64) -> bool {
        let Some(position) = self.entries[..self.length]
            .iter()
            .position(|existing| existing.is_some_and(|value| value.id == id && value.owner == owner))
        else {
            return false;
        };
        for index in position + 1..self.length {
            self.entries[index - 1] = self.entries[index];
        }
        self.length -= 1;
        self.entries[self.length] = None;
        true
    }

    pub fn search(&self, query: &[u8], output: &mut [Hit; MAX_HITS]) -> usize {
        self.search_for(0, query, output)
    }

    pub fn search_for(&self, owner: u64, query: &[u8], output: &mut [Hit; MAX_HITS]) -> usize {
        let mut count = 0;
        for entry in self.entries[..self.length].iter().flatten() {
            if entry.owner != 0 && entry.owner != owner {
                continue;
            }
            let score = entry.matches(query);
            if score == 0 {
                continue;
            }
            let hit = Hit {
                id: entry.id,
                kind: entry.kind,
                score,
            };
            let position = output[..count]
                .iter()
                .position(|existing| hit.score > existing.score)
                .unwrap_or(count);
            if position < MAX_HITS {
                if count < MAX_HITS {
                    count += 1;
                }
                for index in (position + 1..count).rev() {
                    output[index] = output[index - 1];
                }
                output[position] = hit;
            }
        }
        count
    }

    pub fn encode(&self, output: &mut [u8; MAX_ENCODED_BYTES]) -> Result<usize, Error> {
        output[..4].copy_from_slice(&MAGIC);
        output[4] = VERSION;
        output[5] = 0;
        output[6..8].copy_from_slice(&(self.length as u16).to_le_bytes());
        let mut cursor = 8;
        for entry in self.entries[..self.length].iter().flatten() {
            output[cursor..cursor + 8].copy_from_slice(&entry.id.to_le_bytes());
            output[cursor + 8..cursor + 16].copy_from_slice(&entry.owner.to_le_bytes());
            output[cursor + 16] = entry.kind as u8;
            output[cursor + 17] = entry.title_len;
            output[cursor + 18..cursor + 18 + MAX_TITLE_BYTES].copy_from_slice(&entry.title);
            cursor += ENTRY_ENCODED_LEN;
        }
        let checksum = checksum(&output[..cursor]);
        output[cursor..cursor + 8].copy_from_slice(&checksum.to_le_bytes());
        Ok(cursor + 8)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() < 16 {
            return Err(Error::TooShort);
        }
        if bytes[..4] != MAGIC {
            return Err(Error::BadMagic);
        }
        if bytes[4] != VERSION {
            return Err(Error::BadVersion);
        }
        let count = u16::from_le_bytes([bytes[6], bytes[7]]) as usize;
        if count > MAX_ENTRIES {
            return Err(Error::BadLength);
        }
        let total = 8 + count * ENTRY_ENCODED_LEN + 8;
        if total != bytes.len() {
            return Err(Error::BadLength);
        }
        let expected = u64::from_le_bytes(bytes[total - 8..].try_into().unwrap());
        if checksum(&bytes[..total - 8]) != expected {
            return Err(Error::BadChecksum);
        }
        let mut index = Self::new();
        let mut cursor = 8;
        for _ in 0..count {
            let id = u64::from_le_bytes(bytes[cursor..cursor + 8].try_into().unwrap());
            let owner = u64::from_le_bytes(bytes[cursor + 8..cursor + 16].try_into().unwrap());
            let Some(kind) = Kind::decode(bytes[cursor + 16]) else {
                return Err(Error::InvalidEntry);
            };
            let title_len = bytes[cursor + 17] as usize;
            if title_len > MAX_TITLE_BYTES {
                return Err(Error::InvalidEntry);
            }
            let title = &bytes[cursor + 18..cursor + 18 + title_len];
            let Some(entry) = Entry::new_owned(owner, id, kind, title) else {
                return Err(Error::InvalidEntry);
            };
            if !index.insert(entry) {
                return Err(Error::BadLength);
            }
            cursor += ENTRY_ENCODED_LEN;
        }
        Ok(index)
    }
}

impl Default for Index {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy)]
struct Term {
    bytes: [u8; MAX_TERM_BYTES],
    len: usize,
}

struct Terms<'a> {
    input: &'a [u8],
    offset: usize,
}

impl<'a> Terms<'a> {
    const fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }
}

impl Iterator for Terms<'_> {
    type Item = Term;

    fn next(&mut self) -> Option<Self::Item> {
        while self.offset < self.input.len() && !is_term_byte(self.input[self.offset]) {
            self.offset += 1;
        }
        let start = self.offset;
        while self.offset < self.input.len() && is_term_byte(self.input[self.offset]) {
            self.offset += 1;
        }
        let length = self.offset - start;
        if length == 0 {
            return None;
        }
        if length > MAX_TERM_BYTES {
            return self.next();
        }
        let mut bytes = [0; MAX_TERM_BYTES];
        for (destination, source) in bytes.iter_mut().zip(&self.input[start..self.offset]) {
            *destination = source.to_ascii_lowercase();
        }
        Some(Term { bytes, len: length })
    }
}

fn is_term_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'
}

fn copy_normalized(input: &[u8], output: &mut [u8]) -> Option<usize> {
    if input.len() > output.len() || core::str::from_utf8(input).is_err() {
        return None;
    }
    for (destination, source) in output.iter_mut().zip(input) {
        *destination = source.to_ascii_lowercase();
    }
    Some(input.len())
}

fn checksum(bytes: &[u8]) -> u64 {
    let mut value = 0xcbf29ce484222325u64;
    for byte in bytes {
        value ^= u64::from(*byte);
        value = value.wrapping_mul(0x100000001b3);
    }
    value
}

#[cfg(test)]
mod tests {
    use super::{Entry, Error, Index, Kind, Hit, MAX_ENCODED_BYTES, MAX_HITS};

    fn entry(id: u64, kind: Kind, title: &[u8]) -> Entry {
        Entry::new(id, kind, title).unwrap()
    }

    #[test]
    fn bounded_index_ranks_term_coverage_stably() {
        let mut index = Index::new();
        assert!(index.insert(entry(1, Kind::Application, b"Woven Terminal")));
        assert!(index.insert(entry(2, Kind::Command, b"Open Terminal Settings")));
        let mut hits = [Hit { id: 0, kind: Kind::File, score: 0 }; MAX_HITS];
        let count = index.search(b"terminal settings", &mut hits);
        assert_eq!(count, 2);
        assert_eq!(hits[0].id, 2);
        assert_eq!(hits[0].score, 2);
        assert_eq!(hits[1].id, 1);
    }

    #[test]
    fn index_rejects_oversized_titles_and_caps_entries() {
        assert!(Entry::new(1, Kind::File, &[b'a'; super::MAX_TITLE_BYTES + 1]).is_none());
        let mut index = Index::new();
        for id in 0..super::MAX_ENTRIES as u64 {
            assert!(index.insert(entry(id, Kind::Document, b"document")));
        }
        assert!(!index.insert(entry(100, Kind::Document, b"overflow")));
    }

    #[test]
    fn private_entries_are_owner_scoped_and_snapshots_are_authenticated() {
        let mut index = Index::new();
        assert!(index.insert(Entry::new_owned(7, 1, Kind::Message, b"private note").unwrap()));
        assert!(index.insert(Entry::new(2, Kind::Document, b"public note").unwrap()));
        let mut hits = [Hit { id: 0, kind: Kind::File, score: 0 }; MAX_HITS];
        assert_eq!(index.search_for(8, b"private", &mut hits), 0);
        assert_eq!(index.search_for(7, b"private", &mut hits), 1);
        let mut bytes = [0; MAX_ENCODED_BYTES];
        let length = index.encode(&mut bytes).unwrap();
        let decoded = Index::decode(&bytes[..length]).unwrap();
        assert_eq!(decoded.search_for(7, b"note", &mut hits), 2);
        bytes[10] ^= 1;
        assert!(matches!(Index::decode(&bytes[..length]), Err(Error::BadChecksum)));
    }
}
