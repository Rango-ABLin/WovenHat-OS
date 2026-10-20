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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: u64,
    pub kind: Kind,
    pub title: [u8; MAX_TITLE_BYTES],
    pub title_len: u8,
    terms: [[u8; MAX_TERM_BYTES]; MAX_TERMS_PER_ENTRY],
    term_lengths: [u8; MAX_TERMS_PER_ENTRY],
    term_count: u8,
}

impl Entry {
    pub fn new(id: u64, kind: Kind, title: &[u8]) -> Option<Self> {
        let mut entry = Self {
            id,
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

    pub fn search(&self, query: &[u8], output: &mut [Hit; MAX_HITS]) -> usize {
        let mut count = 0;
        for entry in self.entries[..self.length].iter().flatten() {
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

#[cfg(test)]
mod tests {
    use super::{Entry, Index, Kind, Hit, MAX_HITS};

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
}
