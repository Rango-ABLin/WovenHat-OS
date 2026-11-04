//! Bounded file-chooser state machine.
//!
//! Providers supply already-validated display names and opaque entry IDs.
//! This module owns only deterministic selection/navigation and never opens a
//! path or grants filesystem authority by itself.

pub const MAX_ENTRIES: usize = 64;
pub const MAX_NAME_BYTES: usize = 96;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: u64,
    name: [u8; MAX_NAME_BYTES],
    name_len: u8,
    pub directory: bool,
}

impl Entry {
    pub fn new(id: u64, name: &str, directory: bool) -> Option<Self> {
        if name.is_empty() || name.len() > MAX_NAME_BYTES || !name.is_char_boundary(name.len()) {
            return None;
        }
        let mut bytes = [0; MAX_NAME_BYTES];
        bytes[..name.len()].copy_from_slice(name.as_bytes());
        Some(Self { id, name: bytes, name_len: name.len() as u8, directory })
    }

    pub fn name(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len as usize]).unwrap_or("")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { Full, InvalidEntry }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileDialog {
    entries: [Option<Entry>; MAX_ENTRIES],
    count: usize,
    selected: Option<usize>,
    open: bool,
}

impl Default for FileDialog {
    fn default() -> Self { Self::new() }
}

impl FileDialog {
    pub const fn new() -> Self {
        Self { entries: [None; MAX_ENTRIES], count: 0, selected: None, open: true }
    }

    pub const fn is_open(self) -> bool { self.open }
    pub const fn len(self) -> usize { self.count }
    pub const fn is_empty(self) -> bool { self.count == 0 }
    pub fn selected(&self) -> Option<Entry> { self.selected.and_then(|i| self.entries[i]) }

    pub fn push(&mut self, entry: Entry) -> Result<(), Error> {
        if self.count == MAX_ENTRIES { return Err(Error::Full); }
        let Some(slot) = self.entries.iter().position(Option::is_none) else { return Err(Error::Full) };
        self.entries[slot] = Some(entry);
        self.count += 1;
        if self.selected.is_none() { self.selected = Some(slot); }
        Ok(())
    }

    pub fn select_next(&mut self) -> Option<Entry> {
        self.move_selection(1)
    }

    pub fn select_previous(&mut self) -> Option<Entry> {
        self.move_selection(-1)
    }

    pub fn cancel(&mut self) { self.open = false; }

    fn move_selection(&mut self, direction: isize) -> Option<Entry> {
        if self.count == 0 { return None; }
        let start = self.selected.unwrap_or(0);
        for step in 1..=MAX_ENTRIES {
            let index = if direction > 0 {
                (start + step) % MAX_ENTRIES
            } else {
                (start + MAX_ENTRIES - (step % MAX_ENTRIES)) % MAX_ENTRIES
            };
            if self.entries[index].is_some() {
                self.selected = Some(index);
                return self.entries[index];
            }
        }
        self.selected()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_is_bounded_and_deterministic() {
        let mut dialog = FileDialog::new();
        dialog.push(Entry::new(10, "Documents", true).unwrap()).unwrap();
        dialog.push(Entry::new(11, "notes.txt", false).unwrap()).unwrap();
        assert_eq!(dialog.selected().unwrap().id, 10);
        assert_eq!(dialog.select_next().unwrap().id, 11);
        assert_eq!(dialog.select_next().unwrap().id, 10);
        assert_eq!(dialog.select_previous().unwrap().id, 11);
        dialog.cancel();
        assert!(!dialog.is_open());
    }

    #[test]
    fn invalid_and_over_capacity_entries_fail_closed() {
        assert!(Entry::new(1, "", false).is_none());
        assert!(Entry::new(1, "x".repeat(MAX_NAME_BYTES + 1).as_str(), false).is_none());
        let mut dialog = FileDialog::new();
        for id in 0..MAX_ENTRIES as u64 {
            dialog.push(Entry::new(id, "x", false).unwrap()).unwrap();
        }
        assert_eq!(dialog.push(Entry::new(99, "x", false).unwrap()), Err(Error::Full));
    }
}
