//! Bounded architectural stop classification for the WovenDev session.
//!
//! This layer translates an already-captured vector/error pair into a GDB
//! stop signal and publishes it to the caller-owned session. It does not run
//! in an interrupt handler, inspect a stack frame, or resume a task.

use crate::gdb_session::Session;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    Breakpoint,
    SingleStep,
    Fault,
    External,
    Unknown,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct StopEvent {
    pub vector: u8,
    pub error_code: Option<u64>,
    pub reason: StopReason,
    pub signal: u8,
}

pub fn classify(vector: u8, error_code: Option<u64>) -> StopEvent {
    let (reason, signal) = match vector {
        1 => (StopReason::SingleStep, 5),
        3 => (StopReason::Breakpoint, 5),
        2 => (StopReason::External, 2),
        0..=31 => (StopReason::Fault, 11),
        _ => (StopReason::Unknown, 7),
    };
    StopEvent {
        vector,
        error_code,
        reason,
        signal,
    }
}

/// Publish an already-captured stop to an active debugger session.
pub fn publish(session: &mut Session, vector: u8, error_code: Option<u64>) -> StopEvent {
    let event = classify(vector, error_code);
    session.stop(event.signal);
    event
}

#[cfg(any(test, feature = "stage32-12-test"))]
pub fn self_test() -> bool {
    let breakpoint = classify(3, None);
    if breakpoint.reason != StopReason::Breakpoint || breakpoint.signal != 5 {
        return false;
    }
    let page_fault = classify(14, Some(0x05));
    if page_fault.reason != StopReason::Fault
        || page_fault.signal != 11
        || page_fault.error_code != Some(0x05)
    {
        return false;
    }
    let external = classify(2, None);
    if external.reason != StopReason::External || external.signal != 2 {
        return false;
    }
    let unknown = classify(0x80, None);
    if unknown.reason != StopReason::Unknown || unknown.signal != 7 {
        return false;
    }
    let mut session = Session::STOPPED;
    let event = publish(&mut session, 1, None);
    event.reason == StopReason::SingleStep && session.is_stopped()
}

#[cfg(test)]
mod tests {
    use super::{classify, StopReason};

    #[test]
    fn maps_breakpoint_to_trap_signal() {
        let event = classify(3, None);
        assert_eq!(event.reason, StopReason::Breakpoint);
        assert_eq!(event.signal, 5);
    }
}
