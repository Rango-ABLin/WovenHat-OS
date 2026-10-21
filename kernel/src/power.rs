//! Power-off and reset (Stage 13.20; roadmap Stage 25's first two items).
//!
//! ACPI S5 (soft off) is entered by writing the `\_S5` sleep type with
//! SLP_EN to the PM1a (and PM1b) control registers; the machine is handed
//! to ACPI first if the firmware left it in legacy mode. Reset uses the
//! FADT reset register when the firmware says it works, then the keyboard
//! controller, then the chipset reset port, and finally a triple fault.
//! Before either, the live-test log on a WovenHat stick gets a last flush.

use x86_64::instructions::port::Port;

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
    /// Firmware did not advertise S3 or no PM1a control block was found.
    NoSuspend,
    /// A bound driver refused quiescence.
    DriverRefused,
}

/// Driver-quiescence preparation for a future S3 transition. Construction
/// performs no SLP_EN write; the plan must be explicitly resumed or consumed
/// by a later transition implementation.
pub struct SuspendPreparation {
    plan: crate::driver::SuspendPlan,
}

/// Quiesce the current driver set if firmware advertises S3. This boundary is
/// deliberately separate from entering S3 so wake-vector and per-CPU resume
/// work can be completed before any hardware transition is exposed.
pub fn prepare_suspend() -> Result<SuspendPreparation, Error> {
    let Some(power) = crate::hal::acpi::power() else {
        return Err(Error::NoSuspend);
    };
    if power.pm1a_control == 0 || power.s3.is_none() || !acpi_mode(power) {
        return Err(Error::NoSuspend);
    }
    let Some(plan) = crate::driver::suspend_all() else {
        return Err(Error::DriverRefused);
    };
    Ok(SuspendPreparation { plan })
}

/// Abort a prepared suspend and restore drivers in reverse order.
pub fn abort_suspend(preparation: SuspendPreparation) -> bool {
    crate::driver::resume_all(preparation.plan)
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
