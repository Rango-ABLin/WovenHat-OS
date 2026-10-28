//! Bounded trap-frame snapshot for the WovenDev register context.
//!
//! The live exception path will copy architectural values into this plain
//! value before handing them to the debugger. No reference to an interrupt
//! frame or stack memory escapes this module.

use crate::gdb_debugger::{RegisterContext, REGISTER_COUNT};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CapturedFrame {
    /// GDB slots 0..=15: rax, rbx, rcx, rdx, rsi, rdi, rbp, rsp, r8..r15.
    pub general: [u64; 16],
    pub rip: u64,
    pub rflags: u64,
    pub cs: u64,
    pub ss: u64,
    pub error_code: u64,
    pub vector: u64,
}

impl CapturedFrame {
    pub const ZERO: Self = Self {
        general: [0; 16],
        rip: 0,
        rflags: 0,
        cs: 0,
        ss: 0,
        error_code: 0,
        vector: 0,
    };

    /// Copy the captured values into the fixed Stage 32.10 register order.
    pub fn register_context(self) -> RegisterContext {
        let mut values = [0u64; REGISTER_COUNT];
        values[..16].copy_from_slice(&self.general);
        values[16] = self.rip;
        values[17] = self.rflags;
        values[18] = self.cs;
        values[19] = self.ss;
        values[20] = self.error_code;
        values[21] = self.vector;
        RegisterContext { values }
    }
}

#[cfg(any(test, feature = "stage32-14-test"))]
pub fn self_test() -> bool {
    let mut frame = CapturedFrame::ZERO;
    frame.general[0] = 0x11;
    frame.general[15] = 0xff;
    frame.rip = 0x1234;
    frame.rflags = 0x202;
    frame.cs = 8;
    frame.ss = 16;
    frame.error_code = 5;
    frame.vector = 14;
    let context = frame.register_context();
    context.values[0] == 0x11
        && context.values[15] == 0xff
        && context.values[16] == 0x1234
        && context.values[17] == 0x202
        && context.values[18] == 8
        && context.values[19] == 16
        && context.values[20] == 5
        && context.values[21] == 14
        && context.values[22] == 0
        && context.values[23] == 0
}

#[cfg(test)]
mod tests {
    use super::CapturedFrame;

    #[test]
    fn snapshot_does_not_retain_frame_references() {
        let frame = CapturedFrame::ZERO;
        let context = frame.register_context();
        assert_eq!(context.values[23], 0);
    }
}
