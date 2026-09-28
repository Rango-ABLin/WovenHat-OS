//! Bounded ordering/state for transactional PCI bridge-window programming.
//!
//! Hardware access remains in pci.rs. This module owns only the fixed-capacity
//! transaction description and deepest-first ordering contract.

use super::{bridge::Windows, Address};

pub const MAX_BRIDGES: usize = 64;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct Entry {
    pub address: Address,
    pub header_type: u8,
    pub depth: u8,
    pub windows: Windows,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Capacity,
    Duplicate,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Chain {
    entries: [Option<Entry>; MAX_BRIDGES],
    count: usize,
}

impl Default for Chain {
    fn default() -> Self { Self::new() }
}

impl Chain {
    pub const fn new() -> Self {
        Self { entries: [None; MAX_BRIDGES], count: 0 }
    }

    pub fn push(&mut self, entry: Entry) -> Result<(), Error> {
        if self.entries.iter().flatten().any(|current| current.address == entry.address) {
            return Err(Error::Duplicate);
        }
        if self.count == MAX_BRIDGES { return Err(Error::Capacity); }
        self.entries[self.count] = Some(entry);
        self.count += 1;
        Ok(())
    }

    pub fn sort_deepest_first(&mut self) {
        let mut i = 1usize;
        while i < self.count {
            let item = self.entries[i];
            let mut j = i;
            while j > 0
                && self.entries[j - 1].is_some_and(|previous| {
                    item.is_some_and(|current| previous.depth < current.depth)
                })
            {
                self.entries[j] = self.entries[j - 1];
                j -= 1;
            }
            self.entries[j] = item;
            i += 1;
        }
    }

    pub fn len(&self) -> usize { self.count }

    pub fn get(&self, index: usize) -> Option<Entry> {
        (index < self.count).then(|| self.entries[index]).flatten()
    }
}
