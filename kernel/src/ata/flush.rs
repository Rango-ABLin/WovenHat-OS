//! Safe non-data ATA flush sequencing, shared by hardware and host tests.
use crate::block::Error;

const BUSY: u8 = 1 << 7;
const DATA_REQUEST: u8 = 1 << 3;
const ERRORS: u8 = 1 | (1 << 5);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Legacy,
    Extended,
}

impl Command {
    pub fn from_identify(word83: u16) -> Self {
        // IDENTIFY word 83 is valid only with bits 15:14 == 01. Extended
        // flush additionally requires both 48-bit addressing and FLUSH EXT.
        if word83 & 0xc000 == 0x4000 && word83 & 0x2400 == 0x2400 {
            Self::Extended
        } else {
            // E7 is the baseline command. An unsupported/aborted command is
            // an error, never a successful no-op based on unknown capability.
            Self::Legacy
        }
    }

    pub fn opcode(self) -> u8 {
        match self {
            Self::Legacy => 0xe7,
            Self::Extended => 0xea,
        }
    }
}

/// An already exclusively owned ATA channel. Implementations must not sleep
/// or switch tasks: the production caller holds the existing channel IRQ lock.
pub trait Transport {
    fn status(&mut self) -> u8;
    fn select_master(&mut self);
    fn issue(&mut self, command: u8);
    fn settle(&mut self);
    fn ticks(&mut self) -> u64;
    fn relax(&mut self);
}

struct Budget {
    start: u64,
    last_tick: u64,
    stalled: usize,
    deadline: u64,
    spin_limit: usize,
}

impl Budget {
    fn expired(&mut self, now: u64) -> bool {
        if now.wrapping_sub(self.start) >= self.deadline {
            return true;
        }
        if now == self.last_tick {
            self.stalled = self.stalled.saturating_add(1);
        } else {
            self.last_tick = now;
            self.stalled = 0;
        }
        self.stalled >= self.spin_limit
    }
}

fn wait(io: &mut impl Transport, budget: &mut Budget, completion: bool) -> Result<(), Error> {
    loop {
        let status = io.status();
        if status == 0 || status == 0xff {
            return Err(Error::DeviceFault);
        }
        // While BSY is set, the other status bits are not completion evidence.
        if status & BUSY == 0 {
            if completion {
                return if status & (ERRORS | DATA_REQUEST) == 0 {
                    Ok(())
                } else {
                    Err(Error::DeviceFault)
                };
            }
            // Stale ERR/DF belong to the previous command. A new flush may
            // recover pending cached writes, but cannot interrupt a data phase.
            if status & DATA_REQUEST == 0 {
                return Ok(());
            }
        }
        if budget.expired(io.ticks()) {
            return Err(Error::DeviceFault);
        }
        io.relax();
    }
}

pub fn run(
    io: &mut impl Transport,
    command: Command,
    deadline: u64,
    spin_limit: usize,
) -> Result<(), Error> {
    if deadline == 0 || spin_limit == 0 {
        return Err(Error::DeviceFault);
    }
    let start = io.ticks();
    let mut budget = Budget {
        start,
        last_tick: start,
        stalled: 0,
        deadline,
        spin_limit,
    };
    wait(io, &mut budget, false)?;
    io.select_master();
    io.settle();
    wait(io, &mut budget, false)?;
    io.issue(command.opcode());
    io.settle();
    // Do not blindly reissue after an uncertain timeout or error. Return it
    // to the durability caller; a later explicit flush must reacquire idle.
    wait(io, &mut budget, true)
}
