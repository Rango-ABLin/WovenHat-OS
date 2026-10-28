//! Scheduler safe-point adapter for debugger resume control.
//!
//! The adapter accepts only a saved `UserForkFrame` already owned by the
//! scheduler safe point. It never accepts a userspace pointer and never runs
//! from an interrupt handler.

use crate::{gdb_debugger::RegisterContext, syscall::UserForkFrame};

const RFLAGS_TRAP: u64 = 1 << 8;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ResumeAction {
    Continue,
    Step,
    Kill,
}

fn valid_user_return(frame: &UserForkFrame) -> bool {
    crate::user_address::check_user_range(frame.rip, 1).is_ok()
        && crate::user_address::check_user_range(frame.rsp.saturating_sub(1), 1).is_ok()
        && frame.cs & 3 == 3
        && frame.ss & 3 == 3
}

/// Apply a debugger register context to a scheduler-owned saved frame.
pub fn apply_frame(
    frame: &mut UserForkFrame,
    context: &RegisterContext,
    action: ResumeAction,
) -> bool {
    if !valid_user_return(frame) {
        return false;
    }
    frame.rax = context.values[0];
    frame.rbx = context.values[1];
    frame.rcx = context.values[2];
    frame.rdx = context.values[3];
    frame.rsi = context.values[4];
    frame.rdi = context.values[5];
    frame.rbp = context.values[6];
    frame.rsp = context.values[7];
    frame.r8 = context.values[8];
    frame.r9 = context.values[9];
    frame.r10 = context.values[10];
    frame.r11 = context.values[11];
    frame.r12 = context.values[12];
    frame.r13 = context.values[13];
    frame.r14 = context.values[14];
    frame.r15 = context.values[15];
    frame.rip = context.values[16];
    frame.rflags = match action {
        ResumeAction::Step => context.values[17] | RFLAGS_TRAP,
        ResumeAction::Continue | ResumeAction::Kill => context.values[17] & !RFLAGS_TRAP,
    };
    frame.cs = context.values[18];
    frame.ss = context.values[19];
    valid_user_return(frame)
}

/// Apply the context only when `frame` lies in the current task's owned
/// kernel stack. The caller must invoke this from a scheduler safe point.
///
/// # Safety
///
/// `frame` must be the saved frame for the current task and must not be
/// accessed concurrently. The function independently checks the kernel-stack
/// ownership boundary before dereferencing it.
pub unsafe fn apply_current_frame(
    frame: *mut UserForkFrame,
    context: &RegisterContext,
    action: ResumeAction,
) -> bool {
    if frame.is_null()
        || !crate::task::current_kernel_stack_contains(
            frame as u64,
            core::mem::size_of::<UserForkFrame>(),
        )
    {
        return false;
    }
    // SAFETY: the caller is the scheduler safe point, and the ownership check
    // above proves the frame lies in the current task's kernel stack. No
    // pointer supplied by userspace reaches this function.
    unsafe { apply_frame(&mut *frame, context, action) }
}

#[cfg(any(test, feature = "stage32-integrated-test"))]
pub fn self_test() -> bool {
    let mut frame = UserForkFrame {
        rip: 0x4000_0000_1000,
        rsp: 0x4000_0000_2000,
        cs: 0x1b,
        ss: 0x23,
        rflags: 0x202,
        ..UserForkFrame::default()
    };
    let mut context = RegisterContext::ZERO;
    context.values[0] = 7;
    context.values[16] = 0x4000_0000_3000;
    context.values[17] = 0x202;
    context.values[18] = 0x1b;
    context.values[19] = 0x23;
    if !apply_frame(&mut frame, &context, ResumeAction::Step)
        || frame.rax != 7
        || frame.rip != context.values[16]
        || frame.rflags & RFLAGS_TRAP == 0
    {
        return false;
    }
    apply_frame(&mut frame, &context, ResumeAction::Continue) && frame.rflags & RFLAGS_TRAP == 0
}
