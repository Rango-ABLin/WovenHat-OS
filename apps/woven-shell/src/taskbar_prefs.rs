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

    /// Width of one icon-only application entry.
    pub const fn entry_width(self) -> i32 {
        match self {
            Self::Small => 32,
            Self::Medium => 40,
            Self::Large => 48,
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

/// How long a revealed bar stays up, in kernel ticks, after the pointer
/// last touched the strip or the reveal edge. 100 = one second at the
/// kernel timer's 100 Hz. Callers pass their own hold so tests can pick
/// another value; the shell uses this one.
pub const AUTO_HIDE_HOLD: u64 = 100;

/// Taskbar auto-hide state: one bounded preference and its reveal/hide
/// timing. Pure, so the policy is host-tested; the shell feeds it pointer
/// positions and clock ticks and paints whatever `is_hidden` says.
///
/// The preference is seeded from the installed settings record's
/// `taskbar_auto_hide` value and can be flipped in-session by
/// right-clicking an empty part of the strip. Like pins, it is in-session
/// state: the shell holds `FileRead` and no settings-write path exists,
/// so it resets to the record on the next boot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AutoHide {
    enabled: bool,
    hidden: bool,
    /// Tick after which a revealed bar hides again.
    until: u64,
}

impl AutoHide {
    pub const fn off() -> Self {
        Self {
            enabled: false,
            hidden: false,
            until: 0,
        }
    }

    /// Parse the settings value. Only the exact string `true` enables
    /// auto-hide, and the bar starts hidden; anything else -- including
    /// an absent or malformed value -- leaves it off and visible.
    pub fn from_setting(value: Option<&str>) -> Self {
        match value {
            Some("true") => Self {
                enabled: true,
                hidden: true,
                until: 0,
            },
            _ => Self::off(),
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Whether the bar is painted and hit-testable. Auto-hide off can
    /// never hide it.
    pub fn is_hidden(&self) -> bool {
        self.enabled && self.hidden
    }

    /// A pointer update. `reveal_edge` is the y that reveals a hidden
    /// bar (the screen's bottom row); `strip_top` is the y below which a
    /// revealed bar refreshes its hold. Returns whether visibility
    /// changed, so the caller knows to repaint the strip.
    pub fn pointer(
        &mut self,
        y: i32,
        reveal_edge: i32,
        strip_top: i32,
        now: u64,
        hold: u64,
    ) -> bool {
        if !self.enabled {
            return false;
        }
        if self.hidden {
            if y >= reveal_edge {
                self.hidden = false;
                self.until = now.saturating_add(hold);
                return true;
            }
            return false;
        }
        if y >= strip_top {
            self.until = now.saturating_add(hold);
        }
        false
    }

    /// The frame-clock tick. Open taskbar panels defer hiding until they close.
    /// Returns whether visibility changed.
    pub fn tick(&mut self, now: u64, panel_open: bool) -> bool {
        if panel_open || !self.enabled || self.hidden || now <= self.until {
            return false;
        }
        self.hidden = true;
        true
    }

    /// Right-click on an empty strip: flip the preference. Enabling hides
    /// immediately; disabling always reveals. Returns whether visibility
    /// changed, so the caller knows to repaint the strip.
    pub fn toggle(&mut self) -> bool {
        self.enabled = !self.enabled;
        if self.enabled {
            self.hidden = true;
            return true;
        }
        let was_hidden = self.hidden;
        self.hidden = false;
        was_hidden
    }
}
