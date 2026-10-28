//! GDB debug trap entry — Stage 32.13.
//!
//! An assembly trampoline that captures the complete x86-64 register state at a
//! breakpoint (INT3, vector 3) or single-step debug exception (#DB, vector 1),
//! converts it to a [`RegisterContext`] for the Stage 32.10 dispatcher, and
//! drives a bounded GDB session loop over the Stage 32.9 debug-serial transport.
//!
//! The handler resumes the trapped instruction when a GDB client sends `c`
//! (continue) or `k` (kill), enables the trap flag for `s` (single-step), or
//! times out after [`STUB_POLL_LIMIT`] receive polls with no packet (standalone
//! boot path).
//!
//! # IDT wiring
//!
//! `interrupts::init` installs `debug_entry_address()` on both `idt.debug`
//! (vector 1) and `idt.breakpoint` (vector 3) when the `gdb-entry` feature is
//! active.  Neither vector pushes an error code, so the trampoline layout is
//! identical for both.

#[cfg(feature = "gdb-entry")]
use core::sync::atomic::{AtomicBool, Ordering};

#[cfg(feature = "gdb-entry")]
use core::arch::global_asm;

#[cfg(feature = "gdb-entry")]
use crate::gdb_owner::{AttachmentToken, Owner};
#[cfg(feature = "gdb-entry")]
use crate::irq_lock::IrqMutex;

use crate::gdb_debugger::RegisterContext;

// ── Assembly trampoline ───────────────────────────────────────────────────────

#[cfg(feature = "gdb-entry")]
global_asm!(
    ".global wovenhat_debug_entry",
    "wovenhat_debug_entry:",
    // Save all GPRs in the same order used by the syscall trampoline so that
    // the DebugFrame layout and the UserForkFrame layout are identical.
    "push rax",
    "push rbx",
    "push rcx",
    "push rdx",
    "push rsi",
    "push rdi",   // ← rdi is saved here with its original value
    "push rbp",
    "push r8",
    "push r9",
    "push r10",
    "push r11",
    "push r12",
    "push r13",
    "push r14",
    "push r15",
    // Pass a pointer to the DebugFrame as the first SysV argument.
    "mov rdi, rsp",
    // Align the stack for the System V ABI call convention and save the
    // pre-alignment rsp in r12 so it can be restored after the call.
    // r12's original value is already saved on the stack above.
    "mov r12, rsp",
    "and rsp, -16",
    "call wovenhat_debug_dispatch",
    "mov rsp, r12",
    // Restore all GPRs in reverse push order.
    "pop r15",
    "pop r14",
    "pop r13",
    "pop r12",   // ← restores r12 from the stack frame, not the saved-rsp value
    "pop r11",
    "pop r10",
    "pop r9",
    "pop r8",
    "pop rbp",
    "pop rdi",
    "pop rsi",
    "pop rdx",
    "pop rcx",
    "pop rbx",
    "pop rax",
    "iretq",
);

// ── DebugFrame layout ─────────────────────────────────────────────────────────

/// x86-64 trap frame captured by the assembly trampoline.
///
/// Field order mirrors the push sequence: r15 is at the lowest stack address
/// (last pushed), rax is at the highest GPR address (first pushed), followed
/// by the CPU-pushed exception frame (rip / cs / rflags / rsp / ss).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DebugFrame {
    // Saved GPRs — last-pushed first
    pub r15:    u64,
    pub r14:    u64,
    pub r13:    u64,
    pub r12:    u64,
    pub r11:    u64,
    pub r10:    u64,
    pub r9:     u64,
    pub r8:     u64,
    pub rbp:    u64,
    pub rdi:    u64,
    pub rsi:    u64,
    pub rdx:    u64,
    pub rcx:    u64,
    pub rbx:    u64,
    pub rax:    u64,
    // CPU-pushed exception frame (no error code for INT3 / #DB)
    pub rip:    u64,
    pub cs:     u64,
    pub rflags: u64,
    pub rsp:    u64,
    pub ss:     u64,
}

impl DebugFrame {
    /// Convert to a [`RegisterContext`] in GDB x86-64 register order.
    ///
    /// The standard 24-register order used by [`crate::gdb_debugger`] is:
    /// rax rbx rcx rdx rsi rdi rbp rsp r8–r15 rip rflags cs ss ds es fs gs.
    /// The segment registers ds/es/fs/gs are not available from the exception
    /// frame; their slots are zeroed.
    pub fn to_register_context(&self) -> RegisterContext {
        let mut ctx = RegisterContext::ZERO;
        ctx.values[0]  = self.rax;
        ctx.values[1]  = self.rbx;
        ctx.values[2]  = self.rcx;
        ctx.values[3]  = self.rdx;
        ctx.values[4]  = self.rsi;
        ctx.values[5]  = self.rdi;
        ctx.values[6]  = self.rbp;
        ctx.values[7]  = self.rsp;
        ctx.values[8]  = self.r8;
        ctx.values[9]  = self.r9;
        ctx.values[10] = self.r10;
        ctx.values[11] = self.r11;
        ctx.values[12] = self.r12;
        ctx.values[13] = self.r13;
        ctx.values[14] = self.r14;
        ctx.values[15] = self.r15;
        ctx.values[16] = self.rip;
        ctx.values[17] = self.rflags;
        ctx.values[18] = self.cs;
        ctx.values[19] = self.ss;
        // values[20..24]: ds, es, fs, gs — not in the exception frame; zero.
        ctx
    }
}

// ── Stub-loop parameters ──────────────────────────────────────────────────────

/// Maximum receive polls before resuming without a GDB client.
///
/// Each poll is one non-blocking COM2 read (~1 µs in QEMU).  65536 polls ≈
/// 64 ms headroom; a real client responding within ~10 ms will be handled.
/// Boot-time self-tests without a client time out and resume in < 1 ms.
#[cfg(feature = "gdb-entry")]
const STUB_POLL_LIMIT: usize = 65536;

/// Maximum RSP frame size: `$` + 512 data + `#` + 2 = 516 bytes + headroom.
#[cfg(feature = "gdb-entry")]
const FRAME_BUF: usize = 520;

// ── Handler dispatch ──────────────────────────────────────────────────────────

/// Signals that the debug handler has been entered at least once.
/// Used by the self-test to verify the IDT wiring without a GDB client.
#[cfg(feature = "gdb-entry")]
static HANDLER_FIRED: AtomicBool = AtomicBool::new(false);

// ── Single-attachment authority (Stage 32.18) ──────────────────────────────

/// The live trap entry's single GDB attachment, plus the task it belongs to.
///
/// `gdb_owner::Owner` deliberately "owns no global state" (see its module
/// doc), so the static holder lives here, in the module that actually drives
/// a session across trap boundaries. The stored task id is this module's own
/// bookkeeping: `AttachmentToken`'s fields are private, so there is no way to
/// ask the token itself "which task is this" — only to compare it for
/// equality against a token this module already holds. Keeping the task id
/// alongside it lets a trap on an unrelated task recognize that the floor is
/// held by someone else and back off, instead of silently reusing a session
/// that was never its own.
///
/// Rank 1: always acquired and released standalone, never nested with
/// `SCHEDULER` (10), `PROCESS_TABLE` (20), or `PAGING` (10)/`COW_TABLE` (30),
/// so its numeric rank does not interact with that discipline.
#[cfg(feature = "gdb-entry")]
static GDB_OWNER: IrqMutex<(Owner, Option<(AttachmentToken, u64)>)> =
    IrqMutex::with_rank((Owner::new(), None), 1);

/// Decide whether this trap may drive the shared COM2 transport, and with
/// which token. Returns `None` when there is no current task (scheduler not
/// running) or when a different task already holds the floor — the caller
/// must then resume without touching `debug_serial` at all.
#[cfg(feature = "gdb-entry")]
fn gdb_owner_acquire() -> Option<AttachmentToken> {
    // Task slot zero is a valid scheduler identity, while Owner reserves zero
    // as the invalid sentinel. Encode the live identity one-based at this
    // boundary so the production trap path can bind the idle/first task too.
    let task_id = crate::task::current_task_id_if_running()?
        .as_u64()
        .checked_add(1)?;
    gdb_owner_acquire_for(task_id)
}

/// Acquire the debugger floor for a validated task identity.
///
/// The live trap path obtains the identity from the scheduler.  The stage
/// boot self-test runs before a task is marked running, so it uses this same
/// state transition with a deterministic synthetic identity instead of
/// weakening the ownership check or depending on scheduler timing.
#[cfg(feature = "gdb-entry")]
fn gdb_owner_acquire_for(task_id: u64) -> Option<AttachmentToken> {
    if task_id == 0 {
        return None;
    }
    let mut guard = GDB_OWNER.lock();
    let (owner, stored) = &mut *guard;
    if let Some((token, stored_task)) = *stored {
        if stored_task == task_id && owner.matches(token) {
            return Some(token);
        }
        return None;
    }
    let token = owner.attach(task_id)?;
    *stored = Some((token, task_id));
    Some(token)
}

/// Release this trap's attachment unless another successful `attach` has
/// already replaced it (only possible if this module's own bookkeeping were
/// bypassed, which it never is — kept as a defensive check, not a race this
/// function needs to win).
#[cfg(feature = "gdb-entry")]
fn gdb_owner_release(token: AttachmentToken) {
    let mut guard = GDB_OWNER.lock();
    let (owner, stored) = &mut *guard;
    if stored.is_some_and(|(stored_token, _)| stored_token == token) {
        owner.detach(token);
        *stored = None;
    }
}

/// Trap-frame register constants for single-step (RF and TF bits in rflags).
#[cfg(feature = "gdb-entry")]
const RFLAGS_TF: u64 = 1 << 8;
/// The resume flag clears TF-related debug exceptions on re-entry; setting it
/// when resuming from #DB prevents an immediate re-trap on the same instruction.
#[cfg(feature = "gdb-entry")]
const RFLAGS_RF: u64 = 1 << 16;

/// C-callable dispatcher invoked by the assembly trampoline.
///
/// # Safety
///
/// Must only be called from `wovenhat_debug_entry`.  `frame` is the kernel
/// stack pointer at the trap site, valid for the duration of this call.
#[cfg(feature = "gdb-entry")]
#[unsafe(no_mangle)]
unsafe extern "C" fn wovenhat_debug_dispatch(frame: *mut DebugFrame) {
    // SAFETY: frame is set by the trampoline to the kernel stack at the trap
    // entry point, and remains valid until iretq is executed on return.
    let frame = unsafe { &mut *frame };

    HANDLER_FIRED.store(true, Ordering::Release);

    let mut ctx = frame.to_register_context();

    let mut session = crate::gdb_session::Session::STOPPED;
    // Determine the vector from the instruction pointer context.
    // Both #DB (vector 1) and INT3 (vector 3) land here; classify accordingly.
    // The vector cannot be recovered from the exception frame alone; we use the
    // trap-flag bit in rflags to distinguish single-step from breakpoint.
    let vector: u8 = if frame.rflags & RFLAGS_TF != 0 { 1 } else { 3 };
    crate::gdb_trap::publish(&mut session, vector, None);

    // Decide who may drive the shared COM2 transport before reading a single
    // byte from it. A trap on a task that does not hold the floor (a
    // concurrent trap on another CPU, or the scheduler not running yet) must
    // not consume bytes meant for someone else's session.
    let Some(token) = gdb_owner_acquire() else {
        return;
    };

    // Bounded stub loop: receive RSP packets from COM2 and dispatch through the
    // session.  Exits when the session says Continue, Step, Detach, or when
    // STUB_POLL_LIMIT polls elapse with no complete packet. Only Continue and
    // Step leave the attachment in place for a later trap to resume; every
    // other exit (Detach, Kill, or a timeout) releases it below.
    let mut rx_buf = [0u8; FRAME_BUF];
    let mut rx_pos = 0usize;
    let mut reply_buf = [0u8; FRAME_BUF];
    let mut persist_attachment = false;

    'outer: for _ in 0..STUB_POLL_LIMIT {
        let Some(byte) = crate::debug_serial::try_read() else {
            continue;
        };
        if rx_pos < FRAME_BUF {
            rx_buf[rx_pos] = byte;
            rx_pos += 1;
        }
        let Some(pkt) = crate::gdb_stub::decode(&rx_buf[..rx_pos]) else {
            continue;
        };
        rx_pos = 0;

        // A bounded, previously-impossible real read: resolve exactly the
        // requested address range from the current task's own address space
        // when the packet is a well-formed, in-budget `m<addr>,<len>`. Any
        // failure (no address space, unmapped page, oversized request) falls
        // back to today's behavior of an empty window, which `dispatch`
        // already turns into `E01`.
        let mut memory_buf = [0u8; crate::gdb_debugger::MAX_MEMORY_READ];
        let (memory_base, memory_len) = crate::gdb_debugger::parse_memory_request(pkt.data())
            .filter(|&(_, length)| length <= crate::gdb_debugger::MAX_MEMORY_READ)
            .and_then(|(address, length)| {
                let space = crate::task::current_user_address_space()?;
                crate::userspace::read_user_memory(space, address, &mut memory_buf[..length])
                    .then_some((address, length))
            })
            .unwrap_or((0, 0));

        let action = session.handle(
            &pkt,
            &mut ctx,
            memory_base,
            &memory_buf[..memory_len],
            &mut reply_buf,
        );

        use crate::gdb_session::SessionAction;
        match action {
            SessionAction::Reply(n) => {
                crate::debug_serial::write_bytes(&reply_buf[..n]);
            }
            SessionAction::Continue => {
                // Clear the trap flag and set the resume flag so execution
                // continues without re-trapping on the same instruction.
                frame.rflags &= !RFLAGS_TF;
                frame.rflags |= RFLAGS_RF;
                persist_attachment = true;
                break 'outer;
            }
            SessionAction::Step => {
                // Set TF so the CPU generates #DB after the next instruction.
                frame.rflags |= RFLAGS_TF;
                frame.rflags |= RFLAGS_RF;
                persist_attachment = true;
                break 'outer;
            }
            SessionAction::Detach(n) => {
                crate::debug_serial::write_bytes(&reply_buf[..n]);
                break 'outer;
            }
            SessionAction::Kill | SessionAction::Ignore => {}
        }
    }
    // Timeout, Kill, or Detach: resume without modifying rflags further, and
    // release the attachment so a later trap (this task resuming, or a
    // different one) can attach fresh.
    if !persist_attachment {
        gdb_owner_release(token);
    }
}

// ── IDT address helper ────────────────────────────────────────────────────────

/// Virtual address of the `wovenhat_debug_entry` trampoline.
///
/// Used by `interrupts::init` to install the handler on vectors 1 and 3.
#[cfg(feature = "gdb-entry")]
pub fn debug_entry_address() -> u64 {
    extern "C" {
        fn wovenhat_debug_entry();
    }
    wovenhat_debug_entry as *const () as u64
}

// ── Boot self-test ────────────────────────────────────────────────────────────

/// Boot-time self-test.
///
/// 1. Verifies the `DebugFrame::to_register_context` mapping with known values.
/// 2. Triggers an `int3` instruction and checks that `HANDLER_FIRED` is set
///    (confirming the IDT wiring and trampoline work end-to-end).
///
/// The handler times out immediately when no GDB client is connected and
/// returns to the instruction after `int3`, so the self-test does not hang.
#[cfg(any(test, feature = "stage32-17-test", feature = "stage32-18-test", feature = "stage32-19-test"))]
pub fn self_test() -> bool {
    // ── Pure mapping test ─────────────────────────────────────────────────────
    let frame = DebugFrame {
        rax: 0xaaaa, rbx: 0xbbbb, rcx: 0xcccc, rdx: 0xdddd,
        rsi: 0x5555, rdi: 0x6666, rbp: 0x7777,
        r8: 0x0808, r9: 0x0909, r10: 0x1010, r11: 0x1111,
        r12: 0x1212, r13: 0x1313, r14: 0x1414, r15: 0x1515,
        rip: 0xffff_8000_0001_0000, rflags: 0x0202,
        cs: 0x08, rsp: 0x7fff_e000, ss: 0x10,
    };
    let ctx = frame.to_register_context();
    if ctx.values[0] != 0xaaaa { return false; }   // rax
    if ctx.values[1] != 0xbbbb { return false; }   // rbx
    if ctx.values[3] != 0xdddd { return false; }   // rdx
    if ctx.values[7] != 0x7fff_e000 { return false; } // rsp
    if ctx.values[15] != 0x1515 { return false; }  // r15
    if ctx.values[16] != 0xffff_8000_0001_0000 { return false; } // rip
    if ctx.values[17] != 0x0202 { return false; }  // rflags
    if ctx.values[18] != 0x08 { return false; }    // cs
    if ctx.values[19] != 0x10 { return false; }    // ss
    // ds/es/fs/gs (values[20..24]) must be zero
    if ctx.values[20] != 0 { return false; }
    if ctx.values[23] != 0 { return false; }

    // ── Live INT3 test (QEMU only; skipped in host unit-test builds) ─────────
    #[cfg(not(test))]
    {
        HANDLER_FIRED.store(false, Ordering::SeqCst);
        // SAFETY: INT3 is a standard debug instruction; the GDB entry handler
        // has been installed in `interrupts::init` before this self-test runs.
        unsafe { core::arch::asm!("int3", options(nomem, nostack)); }
        // The handler fires, polls COM2 briefly (no client → timeout), returns.
        if !HANDLER_FIRED.load(Ordering::SeqCst) {
            return false;
        }
    }
    true
}

/// Stage 32.18 boot-time self-test for the live owner/binding attachment.
///
/// 1. A fresh acquire followed by a second acquire from the same (current)
///    task must reuse the same token (continuing-session behavior).
/// 2. Manually installing a foreign task's attachment must make this task's
///    acquire back off (`None`), proving the cross-task/cross-CPU floor
///    check works without needing a second real task.
/// 3. Releasing clears the floor so this task can acquire fresh again.
#[cfg(any(test, feature = "stage32-18-test"))]
pub fn owner_binding_self_test() -> bool {
    let task_id = crate::task::current_task_id_if_running()
        .map(|id| id.as_u64())
        .filter(|&id| id != 0)
        .unwrap_or(1);

    let Some(token_a) = gdb_owner_acquire_for(task_id) else {
        return false;
    };
    let Some(token_b) = gdb_owner_acquire_for(task_id) else {
        return false;
    };
    if token_a != token_b {
        return false;
    }
    gdb_owner_release(token_a);

    let foreign_task = if task_id == 1 { 2 } else { 1 };
    {
        let mut guard = GDB_OWNER.lock();
        let (owner, stored) = &mut *guard;
        let Some(foreign_token) = owner.attach(foreign_task) else {
            return false;
        };
        *stored = Some((foreign_token, foreign_task));
    }
    if gdb_owner_acquire_for(task_id).is_some() {
        // Must not have been granted another task's floor. Clean up the
        // manually-installed foreign attachment before failing, so a failed
        // run of this self-test cannot wedge a later one.
        let mut guard = GDB_OWNER.lock();
        let (owner, stored) = &mut *guard;
        if let Some((token, _)) = stored.take() {
            owner.detach(token);
        }
        return false;
    }
    {
        let mut guard = GDB_OWNER.lock();
        let (owner, stored) = &mut *guard;
        let Some((token, _)) = stored.take() else {
            return false;
        };
        if !owner.detach(token) {
            return false;
        }
    }

    let Some(token_c) = gdb_owner_acquire_for(task_id) else {
        return false;
    };
    gdb_owner_release(token_c);
    gdb_owner_release(token_c);
    true
}

// ── Host unit tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::DebugFrame;

    fn make_frame() -> DebugFrame {
        DebugFrame {
            rax: 1, rbx: 2, rcx: 3, rdx: 4,
            rsi: 5, rdi: 6, rbp: 7,
            r8: 8, r9: 9, r10: 10, r11: 11,
            r12: 12, r13: 13, r14: 14, r15: 15,
            rip: 0x1000, rflags: 0x202, cs: 0x08, rsp: 0xf000, ss: 0x10,
        }
    }

    #[test]
    fn gpr_mapping() {
        let ctx = make_frame().to_register_context();
        assert_eq!(ctx.values[0], 1);   // rax
        assert_eq!(ctx.values[1], 2);   // rbx
        assert_eq!(ctx.values[7], 0xf000); // rsp (from exception frame)
        assert_eq!(ctx.values[15], 15); // r15
    }

    #[test]
    fn rip_rflags_segs() {
        let ctx = make_frame().to_register_context();
        assert_eq!(ctx.values[16], 0x1000); // rip
        assert_eq!(ctx.values[17], 0x202);  // rflags
        assert_eq!(ctx.values[18], 0x08);   // cs
        assert_eq!(ctx.values[19], 0x10);   // ss
        // ds/es/fs/gs zeroed
        for i in 20..24 {
            assert_eq!(ctx.values[i], 0, "register {i} should be zero");
        }
    }
}
