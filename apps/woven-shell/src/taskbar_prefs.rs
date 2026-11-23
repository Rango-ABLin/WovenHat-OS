//! Bounded taskbar preferences. The taskbar is anchored to the bottom edge;
//! these settings only choose how its application group is placed and how
//! large each entry is. Values are parsed from the installed settings record
//! and any unknown or malformed value falls back to its default, so a damaged
//! file can never change the layout into an unsupported state.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alignment {
    /// Start button and entries centred horizontally (the default).
    Center,
    /// Start button and entries at the left edge of the taskbar.
    Left,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconSize {
    Small,
    Medium,
    Large,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TaskbarPrefs {
    pub alignment: Alignment,
    pub icon_size: IconSize,
}

impl Alignment {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "center" => Some(Self::Center),
            "left" => Some(Self::Left),
            _ => None,
        }
    }
}

impl IconSize {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "small" => Some(Self::Small),
            "medium" => Some(Self::Medium),
            "large" => Some(Self::Large),
            _ => None,
        }
    }

    /// Width of one application entry. `Medium` is the accepted 48 px entry.
    pub const fn entry_width(self) -> i32 {
        match self {
            Self::Small => 40,
            Self::Medium => 48,
            Self::Large => 56,
        }
    }
}

impl TaskbarPrefs {
    pub const DEFAULT: Self = Self {
        alignment: Alignment::Center,
        icon_size: IconSize::Medium,
    };

    /// Build preferences from raw settings values. Absent or unrecognized
    /// values select the default for that field only.
    pub fn from_values(alignment: Option<&str>, icon_size: Option<&str>) -> Self {
        Self {
            alignment: alignment
                .and_then(Alignment::parse)
                .unwrap_or(Self::DEFAULT.alignment),
            icon_size: icon_size
                .and_then(IconSize::parse)
                .unwrap_or(Self::DEFAULT.icon_size),
        }
    }
}
