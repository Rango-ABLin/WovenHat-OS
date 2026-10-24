//! Power-off and reset (Stage 13.20; roadmap Stage 25's first two items).
//!
//! ACPI S5 (soft off) is entered by writing the `\_S5` sleep type with
//! SLP_EN to the PM1a (and PM1b) control registers; the machine is handed
//! to ACPI first if the firmware left it in legacy mode. Reset uses the
//! FADT reset register when the firmware says it works, then the keyboard
//! controller, then the chipset reset port, and finally a triple fault.
//! Before either, the live-test log on a WovenHat stick gets a last flush.

#[cfg(feature = "stage25-12-test")]
use core::sync::atomic::{AtomicBool, Ordering};
use x86_64::instructions::port::Port;
use x86_64::registers::{
    control::{Cr0, Cr3, Cr4},
    model_specific::Efer,
};
use x86_64::{structures::DescriptorTablePointer, VirtAddr};

/// PM1 control: SCI_EN (ACPI mode), SLP_TYP (bits 10-12), SLP_EN.
const SCI_EN: u16 = 1;
const SLP_TYP_SHIFT: u16 = 10;
const SLP_TYP_MASK: u16 = 0x7 << SLP_TYP_SHIFT;
const SLP_EN: u16 = 1 << 13;
/// Polls while waiting for the machine to act on a power request.
const SETTLE_POLLS: usize = 2_000_000;

/// Why the machine is still running.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// ACPI discovery found no FADT, PM1 control block or `\_S5`.
    NoAcpi,
    /// The firmware would not hand the machine to ACPI.
    AcpiModeRefused,
    /// The request was made and the machine kept running.
    StillRunning,
}

/// Result of attempting the prepared S3 transition. A successful hardware
/// transition does not return; a returned error means the machine remained
/// running and the preparation rollback was attempted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SuspendError {
    NoSuspend,
    AcpiModeRefused,
    StillRunning,
    RollbackFailed,
    AlreadyPending,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrepareError {
    /// Firmware did not advertise S3 or no PM1a control block was found.
    NoSuspend,
    /// A bound driver refused quiescence.
    DriverRefused,
    /// The low-memory wake image could not be installed.
    TrampolineUnavailable,
    /// The firmware FACS could not accept the reserved wake trampoline.
    WakeVectorUnavailable,
    /// A secondary CPU could not reach the scheduler-safe suspend barrier.
    CpuCoordinationUnavailable,
}

/// Driver-quiescence preparation for a future S3 transition. Construction
/// performs no SLP_EN write; the plan must be explicitly resumed or consumed
/// by a later transition implementation.
pub struct SuspendPreparation {
    plan: crate::driver::SuspendPlan,
    trampoline: crate::smp::WakeTrampoline,
    wake_vector: crate::hal::acpi::FacsWakeVector,
    cpu_barrier: crate::smp::CpuSuspendBarrier,
    context: CpuResumeContext,
    resume_frame: u64,
}

/// The one BSP-owned preparation record that survives the firmware transition
/// boundary. It is never populated from userspace and is consumed only by the
/// wake continuation or an explicit failed-entry rollback.
static PENDING_SUSPEND: spin::Mutex<Option<SuspendPreparation>> = spin::Mutex::new(None);

/// CPU state that must remain available to the post-firmware wake path.
/// Pointers are recorded as integers so this value can be copied into a
/// firmware-independent wake record without creating Rust references to
/// state that firmware may temporarily replace.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CpuResumeContext {
    pub cr0: u64,
    pub cr3: u64,
    pub cr4: u64,
    pub efer: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub gdt_base: u64,
    pub gdt_limit: u16,
    pub idt_base: u64,
    pub idt_limit: u16,
}

impl CpuResumeContext {
    fn valid(self) -> bool {
        const CR0_PG: u64 = 1 << 31;
        const CR4_PAE: u64 = 1 << 5;
        const EFER_LMA: u64 = 1 << 10;
        self.cr0 & CR0_PG != 0
            && self.cr4 & CR4_PAE != 0
            && self.efer & EFER_LMA != 0
            && self.cr3 & 0xfff == 0
            && VirtAddr::try_new(self.rsp).is_ok()
            && VirtAddr::try_new(self.gdt_base).is_ok()
            && VirtAddr::try_new(self.idt_base).is_ok()
            && self.gdt_limit != 0
            && self.idt_limit != 0
            && self.rflags & 2 != 0
    }
}

fn capture_cpu_resume_context() -> CpuResumeContext {
    let (cr3, _) = Cr3::read();
    let mut gdt = DescriptorTablePointer {
        limit: 0,
        base: VirtAddr::zero(),
    };
    let mut idt = DescriptorTablePointer {
        limit: 0,
        base: VirtAddr::zero(),
    };
    let rsp;
    let rflags;
    // SAFETY: SGDT/SIDT and reading RSP are side-effect-free snapshots into
    // kernel-owned local storage while the caller controls interrupts.
    unsafe {
        core::arch::asm!("sgdt [{}]", in(reg) &mut gdt, options(nostack, preserves_flags));
        core::arch::asm!("sidt [{}]", in(reg) &mut idt, options(nostack, preserves_flags));
        core::arch::asm!("mov {}, rsp", out(reg) rsp, options(nomem, nostack, preserves_flags));
        core::arch::asm!("pushfq; pop {}", out(reg) rflags, options(nomem, preserves_flags));
    }
    CpuResumeContext {
        cr0: Cr0::read().bits(),
        cr3: cr3.start_address().as_u64(),
        cr4: Cr4::read().bits(),
        efer: Efer::read().bits(),
        rflags,
        rsp,
        gdt_base: gdt.base.as_u64(),
        gdt_limit: gdt.limit,
        idt_base: idt.base.as_u64(),
        idt_limit: idt.limit,
    }
}

/// Restore a validated architectural context and transfer control without a
/// Rust return. The caller must have validated the context and must provide a
/// kernel-owned continuation with the same privilege and address-space rules.
#[expect(
    unreachable_code,
    reason = "Rust requires a diverging tail after noreturn inline assembly"
)]
unsafe fn restore_cpu_resume_context(
    context: &CpuResumeContext,
    continuation: extern "C" fn() -> !,
) -> ! {
    // SAFETY: the caller supplies a validated kernel-owned context. The
    // instruction sequence disables interrupts while control registers and
    // descriptor tables are replaced, then restores the saved flags before
    // jumping to the kernel continuation on the saved stack.
    unsafe {
        core::arch::asm!(
            "cli",
            "mov rax, [rdi + 0]",
            "mov cr0, rax",
            "mov rax, [rdi + 16]",
            "mov cr4, rax",
            "mov rax, [rdi + 8]",
            "mov cr3, rax",
            "mov ecx, 0xc0000080",
            "mov rax, [rdi + 24]",
            "mov rdx, rax",
            "shr rdx, 32",
            "wrmsr",
            "sub rsp, 32",
            "mov ax, word ptr [rdi + 56]",
            "mov word ptr [rsp], ax",
            "mov rax, [rdi + 48]",
            "mov [rsp + 2], rax",
            "lgdt [rsp]",
            "mov ax, word ptr [rdi + 72]",
            "mov word ptr [rsp + 10], ax",
            "mov rax, [rdi + 64]",
            "mov [rsp + 12], rax",
            "lidt [rsp + 10]",
            "mov rax, [rdi + 40]",
            "mov rsp, rax",
            "mov rax, [rdi + 32]",
            "push rax",
            "popfq",
            "jmp rsi",
            in("rdi") context,
            in("rsi") continuation,
            options(noreturn),
        );
    }
    loop {
        core::hint::spin_loop();
    }
}

fn s3_control_value(current: u16, sleep_type: u8) -> Option<u16> {
    if sleep_type > 0x7 {
        return None;
    }
    Some((current & !SLP_TYP_MASK) | (u16::from(sleep_type) << SLP_TYP_SHIFT) | SLP_EN)
}

#[cfg(feature = "stage25-11-test")]
pub fn s3_control_self_test() -> bool {
    let current = 0xa5a5_u16;
    let Some(value) = s3_control_value(current, 3) else {
        return false;
    };
    value & SLP_TYP_MASK == 3 << SLP_TYP_SHIFT
        && value & SLP_EN != 0
        && value & !(SLP_TYP_MASK | SLP_EN) == current & !(SLP_TYP_MASK | SLP_EN)
        && s3_control_value(current, 8).is_none()
}

/// Quiesce the current driver set if firmware advertises S3. This boundary is
/// deliberately separate from entering S3 so wake-vector and per-CPU resume
/// work can be completed before any hardware transition is exposed.
pub fn prepare_suspend() -> Result<SuspendPreparation, PrepareError> {
    let Some(power) = crate::hal::acpi::power() else {
        return Err(PrepareError::NoSuspend);
    };
    if power.pm1a_control == 0 || power.s3.is_none() || !acpi_mode(power) {
        return Err(PrepareError::NoSuspend);
    }
    if crate::smp::wake_page().is_none() {
        return Err(PrepareError::NoSuspend);
    }
    let context = capture_cpu_resume_context();
    if !context.valid() {
        return Err(PrepareError::NoSuspend);
    }
    let Some(plan) = crate::driver::suspend_all() else {
        return Err(PrepareError::DriverRefused);
    };
    let Some(trampoline) = crate::smp::install_wake_trampoline(wake_entry as *const () as u64)
    else {
        let _ = crate::driver::resume_all(plan);
        return Err(PrepareError::TrampolineUnavailable);
    };
    let Some(wake_vector) = crate::hal::acpi::install_facs_wake_vector(
        crate::smp::wake_page().expect("wake page checked before preparation"),
    ) else {
        let _ = crate::smp::release_wake_trampoline(trampoline);
        let _ = crate::driver::resume_all(plan);
        return Err(PrepareError::WakeVectorUnavailable);
    };
    let Some(cpu_barrier) = crate::smp::suspend_other_cpus() else {
        let _ = crate::hal::acpi::restore_facs_wake_vector(wake_vector);
        let _ = crate::smp::release_wake_trampoline(trampoline);
        let _ = crate::driver::resume_all(plan);
        return Err(PrepareError::CpuCoordinationUnavailable);
    };
    Ok(SuspendPreparation {
        plan,
        trampoline,
        wake_vector,
        cpu_barrier,
        context,
        resume_frame: 0,
    })
}

/// Abort a prepared suspend and restore drivers in reverse order.
pub fn abort_suspend(preparation: SuspendPreparation) -> bool {
    let context_valid = preparation.context.valid();
    let wake_vector_restored = crate::hal::acpi::restore_facs_wake_vector(preparation.wake_vector);
    let trampoline_released = crate::smp::release_wake_trampoline(preparation.trampoline);
    let cpus_resumed = crate::smp::resume_suspended_cpus(preparation.cpu_barrier);
    context_valid
        && wake_vector_restored
        && trampoline_released
        && cpus_resumed
        && crate::driver::resume_all(preparation.plan)
}

fn take_pending_suspend() -> Option<SuspendPreparation> {
    PENDING_SUSPEND.lock().take()
}

fn abort_pending_suspend() -> bool {
    take_pending_suspend().is_some_and(abort_suspend)
}

/// Request ACPI S3 after every preparation prerequisite has committed. The
/// successful path transfers control to firmware and therefore does not return
/// on real hardware. If firmware ignores the request, the token is rolled back
/// before returning an explicit error; no caller may accidentally continue with
/// drivers suspended or secondary CPUs parked.
pub fn enter_suspend(
    preparation: SuspendPreparation,
    resume_frame: *mut crate::syscall::UserForkFrame,
) -> Result<(), SuspendError> {
    if !crate::task::current_kernel_stack_contains(
        resume_frame as u64,
        core::mem::size_of::<crate::syscall::UserForkFrame>(),
    ) {
        let _ = abort_suspend(preparation);
        return Err(SuspendError::NoSuspend);
    }
    let context = capture_cpu_resume_context();
    if !context.valid() {
        let _ = abort_suspend(preparation);
        return Err(SuspendError::NoSuspend);
    }
    let preparation = SuspendPreparation {
        context,
        resume_frame: resume_frame as u64,
        ..preparation
    };
    {
        let mut pending = PENDING_SUSPEND.lock();
        if pending.is_some() {
            drop(pending);
            let _ = abort_suspend(preparation);
            return Err(SuspendError::AlreadyPending);
        }
        *pending = Some(preparation);
    }
    let Some(power) = crate::hal::acpi::power() else {
        return Err(if abort_pending_suspend() {
            SuspendError::NoSuspend
        } else {
            SuspendError::RollbackFailed
        });
    };
    let Some((sleep_a, sleep_b)) = power.s3 else {
        return Err(if abort_pending_suspend() {
            SuspendError::NoSuspend
        } else {
            SuspendError::RollbackFailed
        });
    };
    if power.pm1a_control == 0 || !acpi_mode(power) {
        return Err(if abort_pending_suspend() {
            SuspendError::AcpiModeRefused
        } else {
            SuspendError::RollbackFailed
        });
    }
    last_flush();
    x86_64::instructions::interrupts::disable();
    // SAFETY: these are the firmware-described ACPI fixed PM1 control ports,
    // and the sleep types were bounded by ACPI discovery.
    let write_values = unsafe {
        let mut pm1a = Port::<u16>::new(power.pm1a_control);
        let pm1a_current = pm1a.read();
        let pm1b_current = if power.pm1b_control != 0 {
            Some(Port::<u16>::new(power.pm1b_control).read())
        } else {
            None
        };
        (
            s3_control_value(pm1a_current, sleep_a),
            pm1b_current.and_then(|current| s3_control_value(current, sleep_b)),
        )
    };
    let write_ok = match write_values {
        (Some(pm1a_value), Some(pm1b_value)) if power.pm1b_control != 0 => {
            unsafe {
                let mut pm1b = Port::<u16>::new(power.pm1b_control);
                pm1b.write(pm1b_value);
                let mut pm1a = Port::<u16>::new(power.pm1a_control);
                pm1a.write(pm1a_value);
            }
            true
        }
        (Some(pm1a_value), None) if power.pm1b_control == 0 => {
            // SAFETY: PM1a is the validated firmware-described control port.
            unsafe { Port::<u16>::new(power.pm1a_control).write(pm1a_value) };
            true
        }
        _ => false,
    };
    x86_64::instructions::interrupts::enable();
    if !write_ok {
        let restored = abort_pending_suspend();
        return Err(if restored {
            SuspendError::NoSuspend
        } else {
            SuspendError::RollbackFailed
        });
    }
    settle();
    let restored = abort_pending_suspend();
    Err(if restored {
        SuspendError::StillRunning
    } else {
        SuspendError::RollbackFailed
    })
}

extern "C" fn wake_resume_continuation() -> ! {
    let Some(preparation) = take_pending_suspend() else {
        crate::serial::write_line(format_args!("[POWER] firmware wake record missing"));
        loop {
            x86_64::instructions::hlt();
        }
    };
    let resume_frame = preparation.resume_frame;
    if !abort_suspend(preparation) {
        crate::serial::write_line(format_args!("[POWER] firmware wake rollback FAILED"));
        loop {
            x86_64::instructions::hlt();
        }
    }
    let frame = resume_frame as *mut crate::syscall::UserForkFrame;
    // SAFETY: the frame was validated against the current task's kernel stack
    // before it was retained and the wake path runs on that same restored
    // task stack. The successful suspend syscall returns zero after wake.
    unsafe {
        (*frame).rax = 0;
        resume_userspace(frame)
    }
}

unsafe fn resume_userspace(frame: *mut crate::syscall::UserForkFrame) -> ! {
    // SAFETY: `frame` is the validated syscall frame retained by the pending
    // suspend record. The existing assembly epilogue pops this exact layout
    // and performs the privilege-checked iretq back to ring 3.
    unsafe {
        core::arch::asm!(
            "mov rsp, {frame}",
            "jmp wovenhat_syscall_resume",
            frame = in(reg) frame,
            options(noreturn),
        )
    }
}

extern "C" fn wake_entry() -> ! {
    let context = PENDING_SUSPEND
        .lock()
        .as_ref()
        .map(|pending| pending.context);
    if let Some(context) = context {
        // SAFETY: the pending record is kernel-owned and was validated before
        // PM1 S3 entry; the continuation performs the owned rollback.
        unsafe { restore_cpu_resume_context(&context, wake_resume_continuation) }
    }
    x86_64::instructions::interrupts::disable();
    crate::gdt::init();
    crate::interrupts::init();
    crate::serial::write_line(format_args!("[POWER] wake without pending suspend record"));
    loop {
        x86_64::instructions::hlt();
    }
}

#[cfg(feature = "stage25-13-test")]
pub fn wake_record_self_test() -> bool {
    PENDING_SUSPEND.lock().is_none()
        && VirtAddr::try_new(wake_entry as *const () as u64).is_ok()
        && VirtAddr::try_new(wake_resume_continuation as *const () as u64).is_ok()
}

#[cfg(feature = "stage25-15-test")]
pub fn suspend_resume_target_self_test() -> bool {
    PENDING_SUSPEND.lock().is_none()
        && !crate::task::current_kernel_stack_contains(
            u64::MAX,
            core::mem::size_of::<crate::syscall::UserForkFrame>(),
        )
}

#[cfg(feature = "stage25-14-test")]
pub fn suspend_admission_self_test() -> bool {
    PENDING_SUSPEND.lock().is_none()
}

#[cfg(feature = "stage25-7-test")]
pub fn wake_trampoline_self_test() -> bool {
    let Some(trampoline) = crate::smp::install_wake_trampoline(wake_entry as *const () as u64)
    else {
        return false;
    };
    crate::smp::release_wake_trampoline(trampoline)
}

#[cfg(feature = "stage25-8-test")]
pub fn resume_context_self_test() -> bool {
    capture_cpu_resume_context().valid()
}

#[cfg(feature = "stage25-12-test")]
static RESTORE_SELF_TEST_REACHED: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "stage25-12-test")]
extern "C" fn restore_self_test_continuation() -> ! {
    RESTORE_SELF_TEST_REACHED.store(true, Ordering::Release);
    crate::serial::write_line(format_args!("[S25.12] CPU context restore: PASSED"));
    crate::qemu_test_exit_success();
    loop {
        x86_64::instructions::hlt();
    }
}

#[cfg(feature = "stage25-12-test")]
pub fn resume_context_restore_self_test() -> bool {
    let context = capture_cpu_resume_context();
    if !context.valid() {
        return false;
    }
    // The continuation never returns: the restored context transfers directly
    // to it and the QEMU test device terminates the guest.
    unsafe { restore_cpu_resume_context(&context, restore_self_test_continuation) }
}

/// Turn the machine off. Returns only if it is still running.
pub fn power_off() -> Error {
    let Some(power) = crate::hal::acpi::power() else {
        return Error::NoAcpi;
    };
    let (Some((sleep_a, sleep_b)), true) = (power.s5, power.pm1a_control != 0) else {
        return Error::NoAcpi;
    };
    crate::serial::write_line(format_args!(
        "[POWER] powering off: PM1a={:#x} PM1b={:#x} S5=({}, {})",
        power.pm1a_control, power.pm1b_control, sleep_a, sleep_b
    ));
    last_flush();
    if !acpi_mode(power) {
        return Error::AcpiModeRefused;
    }
    x86_64::instructions::interrupts::disable();
    // SAFETY: the PM1 control ports are the firmware-described ACPI fixed
    // hardware registers; writing SLP_TYP | SLP_EN requests S5.
    unsafe {
        let mut pm1a = Port::<u16>::new(power.pm1a_control);
        let value = pm1a.read() & !SLP_TYP_MASK;
        if power.pm1b_control != 0 {
            let mut pm1b = Port::<u16>::new(power.pm1b_control);
            let b = pm1b.read() & !SLP_TYP_MASK;
            pm1b.write(b | (u16::from(sleep_b) << SLP_TYP_SHIFT) | SLP_EN);
        }
        pm1a.write(value | (u16::from(sleep_a) << SLP_TYP_SHIFT) | SLP_EN);
    }
    settle();
    x86_64::instructions::interrupts::enable();
    Error::StillRunning
}

/// Reset the machine. Never returns: the last method, a triple fault,
/// always resets.
pub fn reboot() -> ! {
    crate::serial::write_line(format_args!("[POWER] resetting"));
    last_flush();
    x86_64::instructions::interrupts::disable();
    if let Some((space, address, value)) = crate::hal::acpi::power().and_then(|power| power.reset) {
        crate::serial::write_line(format_args!(
            "[POWER] reset register: space {} address {:#x}",
            space, address
        ));
        // System I/O is the common case; memory and PCI configuration
        // reset registers fall through to the methods below.
        if space == 1 {
            if let Ok(port) = u16::try_from(address) {
                // SAFETY: the FADT names this port as the reset register.
                unsafe { Port::<u8>::new(port).write(value) };
                settle();
            }
        }
    }
    crate::serial::write_line(format_args!("[POWER] reset: keyboard controller"));
    // SAFETY: 0xfe to the 8042 command port pulses the CPU reset line.
    unsafe {
        let mut status = Port::<u8>::new(0x64);
        for _ in 0..SETTLE_POLLS {
            if status.read() & 0x02 == 0 {
                break;
            }
        }
        status.write(0xfe);
    }
    settle();
    crate::serial::write_line(format_args!("[POWER] reset: chipset reset control"));
    // SAFETY: 0xcf9 is the PCI reset control register on Intel-compatible
    // chipsets: 0x02 selects a hard reset, 0x06 performs it.
    unsafe {
        let mut control = Port::<u8>::new(0xcf9);
        control.write(0x02);
        control.write(0x06);
    }
    settle();
    crate::serial::write_line(format_args!("[POWER] reset: triple fault"));
    triple_fault()
}

/// Hand the machine to ACPI if the firmware left it in legacy mode.
fn acpi_mode(power: crate::hal::acpi::Power) -> bool {
    // SAFETY: reading the PM1a control register has no side effects.
    let enabled = || unsafe { Port::<u16>::new(power.pm1a_control).read() } & SCI_EN != 0;
    if enabled() {
        return true;
    }
    if power.smi_command == 0 || power.acpi_enable == 0 {
        return false;
    }
    // SAFETY: writing ACPI_ENABLE to SMI_CMD is the FADT-described handoff.
    unsafe { Port::<u8>::new(power.smi_command).write(power.acpi_enable) };
    (0..SETTLE_POLLS).any(|_| enabled())
}

fn settle() {
    for _ in 0..SETTLE_POLLS {
        core::hint::spin_loop();
    }
}

fn last_flush() {
    #[cfg(any(not(feature = "qemu-test"), feature = "stage13-12-test"))]
    if crate::livelog::flush_now() {
        crate::serial::write_line(format_args!("[POWER] live-test log saved"));
    }
}

fn triple_fault() -> ! {
    // An empty interrupt descriptor table: the next exception cannot be
    // delivered, and neither can the double fault it raises.
    let empty = x86_64::structures::DescriptorTablePointer {
        limit: 0,
        base: x86_64::VirtAddr::new(0),
    };
    // SAFETY: deliberately unusable; the CPU resets on the next exception.
    unsafe {
        x86_64::instructions::tables::lidt(&empty);
        core::arch::asm!("int3", options(nomem, nostack));
    }
    loop {
        x86_64::instructions::hlt();
    }
}

/// Stage 13.20 acceptance: ACPI describes power-off and reset, and one of
/// them is carried out. `reboot` selects which; the harness requires QEMU to
/// stop with no failure line after the request.
#[cfg(feature = "stage13-20-test")]
pub fn self_test(reboot_requested: bool) -> bool {
    fn step(name: &str, ok: bool) -> bool {
        crate::serial::write_line(format_args!(
            "[S13.20] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }
    let Some(power) = crate::hal::acpi::power() else {
        return step("ACPI discovery ran", false);
    };
    crate::serial::write_line(format_args!("[S13.20] {:?}", power));
    if !step(
        "a PM1a control block and \\_S5",
        power.pm1a_control != 0 && power.s5.is_some(),
    ) {
        return false;
    }
    if reboot_requested {
        crate::serial::write_line(format_args!("[S13.20] requesting reset"));
        reboot();
    }
    crate::serial::write_line(format_args!("[S13.20] requesting power-off"));
    let error = power_off();
    step("the machine turned off", false);
    crate::serial::write_line(format_args!("[S13.20] still running: {:?}", error));
    false
}
