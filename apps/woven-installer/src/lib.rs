#![cfg_attr(not(test), no_std)]

//! Bounded adapter from the validated installer state model to Ring-3 text.
//!
//! The input is a snapshot, not an installation command. It is deliberately
//! independent of disks, filesystems, partitions, bootloaders, networking,
//! and subprocesses. A future graphical frontend can use the same validated
//! state model without giving this shell destructive authority.

pub const MAX_SNAPSHOT: usize = 256;
pub const MAX_TEXT: usize = 128;
/// Must match the bounded installer IPC request payload.
pub const MAX_PLAN: usize = 192;
pub const MAX_ERROR: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Welcome,
    TargetSelection,
    PartitionReview,
    Settings,
    Progress,
    Cancelling,
    Cancelled,
    Success,
    Failure,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct State<'a> {
    pub screen: Screen,
    pub target: &'a [u8],
    pub plan: &'a [u8],
    pub settings: &'a [u8],
    pub progress: u8,
    pub confirmed: bool,
    pub message: &'a [u8],
    pub error: &'a [u8],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    TooLong,
    MalformedField,
    UnknownField,
    DuplicateField,
    InvalidScreen,
    InvalidBoolean,
    InvalidProgress,
    InvalidText,
    UnvalidatedPlan,
    MissingRequiredField,
}

impl<'a> State<'a> {
    pub const fn welcome() -> Self {
        Self {
            screen: Screen::Welcome,
            target: b"",
            plan: b"validated",
            settings: b"",
            progress: 0,
            confirmed: false,
            message: b"",
            error: b"",
        }
    }

    pub fn parse(snapshot: &'a [u8]) -> Result<Self, ParseError> {
        if snapshot.is_empty() {
            return Err(ParseError::Empty);
        }
        if snapshot.len() > MAX_SNAPSHOT {
            return Err(ParseError::TooLong);
        }
        let mut state = Self::welcome();
        let mut seen = 0u16;
        for field in snapshot.split(|byte| *byte == b';') {
            let Some(separator) = field.iter().position(|byte| *byte == b'=') else {
                return Err(ParseError::MalformedField);
            };
            let key = &field[..separator];
            let value = &field[separator + 1..];
            let bit = match key {
                b"screen" => 1,
                b"target" => 2,
                b"plan" => 4,
                b"settings" => 8,
                b"progress" => 16,
                b"confirmed" => 32,
                b"message" => 64,
                b"error" => 128,
                b"validated" => 256,
                _ => return Err(ParseError::UnknownField),
            };
            if seen & bit != 0 {
                return Err(ParseError::DuplicateField);
            }
            seen |= bit;
            match key {
                b"screen" => state.screen = parse_screen(value)?,
                b"target" => state.target = bounded(value, MAX_TEXT)?,
                b"plan" => state.plan = bounded(value, MAX_PLAN)?,
                b"settings" => state.settings = bounded(value, MAX_TEXT)?,
                b"progress" => state.progress = parse_progress(value)?,
                b"confirmed" => state.confirmed = parse_bool(value)?,
                b"message" => state.message = bounded(value, MAX_TEXT)?,
                b"error" => state.error = bounded(value, MAX_ERROR)?,
                b"validated" if value == b"1" => {}
                b"validated" => return Err(ParseError::UnvalidatedPlan),
                _ => unreachable!(),
            }
        }
        if seen & 256 == 0 || state.plan.is_empty() {
            return Err(ParseError::UnvalidatedPlan);
        }
        if matches!(
            state.screen,
            Screen::TargetSelection
                | Screen::PartitionReview
                | Screen::Settings
                | Screen::Progress
                | Screen::Success
        ) && state.target.is_empty()
        {
            return Err(ParseError::MissingRequiredField);
        }
        if matches!(
            state.screen,
            Screen::Settings | Screen::Progress | Screen::Success
        ) && state.settings.is_empty()
        {
            return Err(ParseError::MissingRequiredField);
        }
        if state.screen == Screen::Failure && state.error.is_empty() {
            return Err(ParseError::MissingRequiredField);
        }
        Ok(state)
    }
}

fn bounded(value: &[u8], limit: usize) -> Result<&[u8], ParseError> {
    if value.is_empty()
        || value.len() > limit
        || value.iter().any(|byte| *byte < 0x20 || *byte == 0x7f)
    {
        return Err(ParseError::InvalidText);
    }
    Ok(value)
}

fn parse_bool(value: &[u8]) -> Result<bool, ParseError> {
    match value {
        b"0" => Ok(false),
        b"1" => Ok(true),
        _ => Err(ParseError::InvalidBoolean),
    }
}

fn parse_progress(value: &[u8]) -> Result<u8, ParseError> {
    if value.is_empty() || value.len() > 3 {
        return Err(ParseError::InvalidProgress);
    }
    let mut result = 0u16;
    for byte in value {
        if !byte.is_ascii_digit() {
            return Err(ParseError::InvalidProgress);
        }
        result = result * 10 + u16::from(*byte - b'0');
    }
    if result > 100 {
        Err(ParseError::InvalidProgress)
    } else {
        Ok(result as u8)
    }
}

fn parse_screen(value: &[u8]) -> Result<Screen, ParseError> {
    match value {
        b"welcome" => Ok(Screen::Welcome),
        b"target-selection" => Ok(Screen::TargetSelection),
        b"partition-review" => Ok(Screen::PartitionReview),
        b"settings" => Ok(Screen::Settings),
        b"progress" => Ok(Screen::Progress),
        b"cancelling" => Ok(Screen::Cancelling),
        b"cancelled" => Ok(Screen::Cancelled),
        b"success" => Ok(Screen::Success),
        b"failure" => Ok(Screen::Failure),
        _ => Err(ParseError::InvalidScreen),
    }
}

#[cfg(test)]
mod tests {
    use super::{ParseError, Screen, State};

    #[test]
    fn parses_each_bounded_snapshot_shape() {
        let state = State::parse(b"screen=progress;target=USB fixture;plan=validated;settings=UTC;progress=42;confirmed=0;validated=1;message=review").unwrap();
        assert_eq!(state.screen, Screen::Progress);
        assert_eq!(state.progress, 42);
        assert!(!state.confirmed);
    }

    #[test]
    fn rejects_unvalidated_or_destructive_payloads() {
        assert_eq!(
            State::parse(b"screen=welcome;plan=untrusted"),
            Err(ParseError::UnvalidatedPlan)
        );
        assert_eq!(
            State::parse(b"screen=progress;plan=validated;validated=1"),
            Err(ParseError::MissingRequiredField)
        );
        assert_eq!(
            State::parse(b"screen=welcome;plan=validated;validated=1;install=/dev/sda"),
            Err(ParseError::UnknownField)
        );
    }

    #[test]
    fn rejects_duplicate_and_unbounded_fields() {
        assert_eq!(
            State::parse(b"screen=welcome;screen=success;plan=validated;validated=1"),
            Err(ParseError::DuplicateField)
        );
        assert_eq!(
            State::parse(b"screen=welcome;plan=validated;validated=1;progress=101"),
            Err(ParseError::InvalidProgress)
        );
    }

    #[test]
    fn accepts_the_full_bounded_ipc_plan_capacity() {
        let mut snapshot = [0_u8; 256];
        let prefix = b"screen=welcome;validated=1;plan=";
        snapshot[..prefix.len()].copy_from_slice(prefix);
        snapshot[prefix.len()..prefix.len() + super::MAX_PLAN].fill(b'x');
        let length = prefix.len() + super::MAX_PLAN;
        let state = State::parse(&snapshot[..length]).expect("maximum plan is valid");
        assert_eq!(state.plan.len(), super::MAX_PLAN);
    }

    #[test]
    fn rejects_a_plan_above_the_ipc_capacity() {
        let mut snapshot = [0_u8; 256];
        let prefix = b"screen=welcome;validated=1;plan=";
        snapshot[..prefix.len()].copy_from_slice(prefix);
        snapshot[prefix.len()..prefix.len() + super::MAX_PLAN + 1].fill(b'x');
        let length = prefix.len() + super::MAX_PLAN + 1;
        assert_eq!(State::parse(&snapshot[..length]), Err(ParseError::InvalidText));
    }
}
