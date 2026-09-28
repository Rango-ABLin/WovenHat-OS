//! Bounded generation-safe PCI interrupt-vector leases for Stage 13.2.
//!
//! The allocator deliberately owns only the WovenHat PCI-device vector band.
//! Architectural exceptions, PIC vectors, syscall 0x80, LAPIC timer/IPI
//! vectors, and the existing Wi-Fi vector are outside this pool.

pub const FIRST_VECTOR: u8 = 0x90;
pub const LAST_VECTOR: u8 = 0xcf;
pub const VECTOR_COUNT: usize = (LAST_VECTOR - FIRST_VECTOR + 1) as usize;
const NO_OWNER: u32 = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lease {
    pub vector: u8,
    generation: u32,
    owner: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidOwner,
    Exhausted,
    InvalidLease,
    NotOwner,
}

#[derive(Clone, Copy)]
struct Slot {
    generation: u32,
    owner: u32,
}

impl Slot {
    const EMPTY: Self = Self { generation: 1, owner: NO_OWNER };
}

pub struct Allocator {
    slots: [Slot; VECTOR_COUNT],
}

impl Allocator {
    pub const fn new() -> Self { Self { slots: [Slot::EMPTY; VECTOR_COUNT] } }

    pub fn allocate(&mut self, owner: u32) -> Result<Lease, Error> {
        if owner == NO_OWNER { return Err(Error::InvalidOwner); }
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if slot.owner == NO_OWNER {
                slot.owner = owner;
                return Ok(Lease {
                    vector: FIRST_VECTOR + index as u8,
                    generation: slot.generation,
                    owner,
                });
            }
        }
        Err(Error::Exhausted)
    }

    pub fn validate(&self, lease: Lease, owner: u32) -> Result<(), Error> {
        if owner == NO_OWNER { return Err(Error::InvalidOwner); }
        let index = lease.vector.checked_sub(FIRST_VECTOR).map(usize::from)
            .filter(|index| *index < VECTOR_COUNT).ok_or(Error::InvalidLease)?;
        let slot = self.slots[index];
        if slot.generation != lease.generation || slot.owner == NO_OWNER {
            return Err(Error::InvalidLease);
        }
        if slot.owner != owner || lease.owner != owner { return Err(Error::NotOwner); }
        Ok(())
    }

    pub fn release(&mut self, lease: Lease, owner: u32) -> Result<(), Error> {
        self.validate(lease, owner)?;
        let index = usize::from(lease.vector - FIRST_VECTOR);
        let slot = &mut self.slots[index];
        slot.owner = NO_OWNER;
        slot.generation = slot.generation.wrapping_add(1);
        if slot.generation == 0 { slot.generation = 1; }
        Ok(())
    }
}
