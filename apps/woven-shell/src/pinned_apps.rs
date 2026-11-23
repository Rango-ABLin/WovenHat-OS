//! Bounded, persistent list of pinned taskbar applications. Pins are stored as
//! a comma-separated list of catalog names in the installed settings record.
//! Parsing is defensive: invalid names and duplicates are dropped, the list is
//! capped, and an absent setting restores the curated default set.

pub const MAX_PINNED: usize = 8;
pub const MAX_NAME: usize = 32;

const DEFAULT_PINS: [&str; 3] = ["WovenFiles", "WovenTerminal", "WovenStore"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PinError {
    Full,
    Invalid,
    Duplicate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PinnedApps {
    names: [[u8; MAX_NAME]; MAX_PINNED],
    lens: [u8; MAX_PINNED],
    count: usize,
}

fn valid_name(name: &str) -> bool {
    (1..=MAX_NAME).contains(&name.len())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

impl PinnedApps {
    pub const fn empty() -> Self {
        Self {
            names: [[0; MAX_NAME]; MAX_PINNED],
            lens: [0; MAX_PINNED],
            count: 0,
        }
    }

    pub fn defaults() -> Self {
        let mut pins = Self::empty();
        for name in DEFAULT_PINS {
            let _ = pins.pin(name);
        }
        pins
    }

    /// Parse a settings value. `None` means the setting is absent and yields the
    /// defaults. An empty string means the user unpinned everything.
    pub fn from_setting(value: Option<&str>) -> Self {
        let Some(value) = value else {
            return Self::defaults();
        };
        let mut pins = Self::empty();
        for name in value.split(',').map(str::trim) {
            if name.is_empty() {
                continue;
            }
            let _ = pins.pin(name);
        }
        pins
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        if index >= self.count {
            return None;
        }
        let length = self.lens[index] as usize;
        core::str::from_utf8(&self.names[index][..length]).ok()
    }

    pub fn contains(&self, name: &str) -> bool {
        (0..self.count).any(|index| self.get(index) == Some(name))
    }

    pub fn pin(&mut self, name: &str) -> Result<(), PinError> {
        if !valid_name(name) {
            return Err(PinError::Invalid);
        }
        if self.contains(name) {
            return Err(PinError::Duplicate);
        }
        if self.count == MAX_PINNED {
            return Err(PinError::Full);
        }
        self.names[self.count][..name.len()].copy_from_slice(name.as_bytes());
        self.lens[self.count] = name.len() as u8;
        self.count += 1;
        Ok(())
    }

    /// Remove a pin, keeping the remaining order. Returns whether it was present.
    pub fn unpin(&mut self, name: &str) -> bool {
        let Some(index) = (0..self.count).find(|&index| self.get(index) == Some(name)) else {
            return false;
        };
        for next in index..self.count - 1 {
            self.names[next] = self.names[next + 1];
            self.lens[next] = self.lens[next + 1];
        }
        self.count -= 1;
        self.names[self.count] = [0; MAX_NAME];
        self.lens[self.count] = 0;
        true
    }
}
