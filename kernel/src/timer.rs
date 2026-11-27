use core::{
    arch::asm,
    sync::atomic::{AtomicU64, Ordering},
};

pub const IRQ: u8 = 0;
pub const FREQUENCY_HZ: u32 = 100;

const PIT_INPUT_HZ: u32 = 1_193_182;
const CHANNEL_0_DATA: u16 = 0x40;
const COMMAND: u16 = 0x43;
const CHANNEL_0_RATE_GENERATOR: u8 = 0x34;

static TICKS: AtomicU64 = AtomicU64::new(0);

pub fn init() {
    let divisor = (PIT_INPUT_HZ / FREQUENCY_HZ) as u16;

    // SAFETY: WovenHat owns the PIT, interrupts are still disabled, and IRQ0
    // remains masked while channel 0 is configured as a rate generator.
    unsafe {
        outb(COMMAND, CHANNEL_0_RATE_GENERATOR);
        outb(CHANNEL_0_DATA, divisor as u8);
        outb(CHANNEL_0_DATA, (divisor >> 8) as u8);
    }
}

pub fn record_tick() {
    let _ = TICKS.try_update(Ordering::Relaxed, Ordering::Relaxed, |ticks| {
        Some(ticks.saturating_add(1))
    });
}

/// The ACPI PM timer: port in the low 16 bits, bit 16 set when it counts
/// 32 bits; 0 before ACPI discovery or when there is none.
static PM_TIMER: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
/// The PM timer's rate.
pub const PM_TIMER_HZ: u64 = 3_579_545;

/// Use the ACPI PM timer at `port` for calibration and deadlines.
pub fn set_pm_timer(port: u16, wide: bool) {
    PM_TIMER.store(u32::from(port) | (u32::from(wide) << 16), Ordering::Release);
}

/// The PM timer's current count and its width mask, if there is one.
pub fn pm_read() -> Option<(u32, u32)> {
    let config = PM_TIMER.load(Ordering::Acquire);
    let port = (config & 0xffff) as u16;
    if port == 0 {
        return None;
    }
    let mask = if config & (1 << 16) != 0 { u32::MAX } else { 0x00ff_ffff };
    // SAFETY: the FADT names this port as the read-only PM timer.
    let value = unsafe { x86_64::instructions::port::Port::<u32>::new(port).read() };
    Some((value & mask, mask))
}

/// A bounded wait measured by the PM timer, or by ticks without one, so a
/// stalled tick cannot stretch a driver's wait into minutes. Poll
/// [`Deadline::passed`] at least every few seconds (the 24-bit PM timer
/// wraps every 4.7 s).
pub struct Deadline {
    start: Option<(u32, u32)>,
    elapsed: u64,
    start_tick: u64,
    limit_ms: u64,
}

impl Deadline {
    pub fn after_ms(limit_ms: u64) -> Self {
        Self { start: pm_read(), elapsed: 0, start_tick: ticks(), limit_ms }
    }

    pub fn passed(&mut self) -> bool {
        match (self.start, pm_read()) {
            (Some((last, mask)), Some((now, _))) => {
                self.elapsed += u64::from(now.wrapping_sub(last) & mask);
                self.start = Some((now, mask));
                self.elapsed * 1000 >= self.limit_ms * PM_TIMER_HZ
            }
            _ => ticks().wrapping_sub(self.start_tick) * 1000 / u64::from(FREQUENCY_HZ) >= self.limit_ms,
        }
    }
}

/// Wait `ms` milliseconds by the PM timer (by ticks without one).
pub fn spin_ms(ms: u64) {
    let mut deadline = Deadline::after_ms(ms);
    while !deadline.passed() {
        core::hint::spin_loop();
    }
}

pub fn ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

/// Write one byte to an x86 I/O port.
///
/// # Safety
///
/// The caller must ensure that `port` is valid and that WovenHat owns the
/// associated device.
unsafe fn outb(port: u16, value: u8) {
    // SAFETY: The caller upholds the port-I/O requirements above.
    unsafe {
        asm!(
            "out dx, al",
            in("dx") port,
            in("al") value,
            options(nomem, nostack, preserves_flags)
        );
    }
}
