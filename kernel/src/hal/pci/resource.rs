//! Bounded transactional PCI resource allocator for Stage 13.2.
//!
//! The allocator is deliberately independent of PCI config-space I/O: callers
//! first reserve every required window, then program hardware. Dropping or
//! rolling back a transaction returns all reservations, so partial assignment
//! cannot leak address space.

pub const MAX_RANGES: usize = 96;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Range {
    pub base: u64,
    pub size: u64,
}

impl Range {
    pub const fn end(self) -> Option<u64> {
        self.base.checked_add(self.size)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidRange,
    InvalidAlignment,
    Capacity,
    Exhausted,
    InvalidReservation,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reservation {
    slot: u8,
    generation: u32,
    pub range: Range,
}

#[derive(Clone, Copy)]
struct Slot {
    occupied: bool,
    generation: u32,
    range: Range,
}

impl Slot {
    #[cfg_attr(not(test), expect(dead_code, reason = "constructed through dormant Stage 13.2 aperture setup until host resources are wired"))]
    const fn empty() -> Self {
        Self { occupied: false, generation: 1, range: Range { base: 0, size: 0 } }
    }
}

pub struct Allocator {
    aperture: Range,
    slots: [Slot; MAX_RANGES],
}

impl Allocator {
    #[cfg_attr(not(test), expect(dead_code, reason = "production constructor becomes live when host bridge apertures are discovered"))]
    pub const fn new(base: u64, size: u64) -> Self {
        Self { aperture: Range { base, size }, slots: [Slot::empty(); MAX_RANGES] }
    }

    pub fn reserve(&mut self, size: u64, alignment: u64) -> Result<Reservation, Error> {
        if size == 0 || self.aperture.size == 0 || self.aperture.end().is_none() {
            return Err(Error::InvalidRange);
        }
        if alignment == 0 || !alignment.is_power_of_two() {
            return Err(Error::InvalidAlignment);
        }
        let slot = self.slots.iter().position(|slot| !slot.occupied).ok_or(Error::Capacity)?;
        let aperture_end = self.aperture.end().ok_or(Error::InvalidRange)?;
        let mut candidate = align_up(self.aperture.base, alignment).ok_or(Error::Exhausted)?;
        loop {
            let end = candidate.checked_add(size).ok_or(Error::Exhausted)?;
            if end > aperture_end {
                return Err(Error::Exhausted);
            }
            let mut next = None;
            for used in self.slots.iter().filter(|slot| slot.occupied) {
                let used_end = used.range.end().ok_or(Error::InvalidRange)?;
                if candidate < used_end && used.range.base < end {
                    next = Some(next.map_or(used_end, |value: u64| value.max(used_end)));
                }
            }
            match next {
                Some(after) => candidate = align_up(after, alignment).ok_or(Error::Exhausted)?,
                None => break,
            }
        }
        let generation = self.slots[slot].generation;
        let range = Range { base: candidate, size };
        self.slots[slot].occupied = true;
        self.slots[slot].range = range;
        Ok(Reservation { slot: slot as u8, generation, range })
    }

    pub fn release(&mut self, reservation: Reservation) -> Result<(), Error> {
        let Some(slot) = self.slots.get_mut(reservation.slot as usize) else {
            return Err(Error::InvalidReservation);
        };
        if !slot.occupied || slot.generation != reservation.generation || slot.range != reservation.range {
            return Err(Error::InvalidReservation);
        }
        slot.occupied = false;
        slot.range = Range { base: 0, size: 0 };
        slot.generation = next_generation(slot.generation);
        Ok(())
    }


    #[cfg(test)]
    pub fn active(&self) -> usize {
        self.slots.iter().filter(|slot| slot.occupied).count()
    }
}

fn align_up(value: u64, alignment: u64) -> Option<u64> {
    let mask = alignment.checked_sub(1)?;
    value.checked_add(mask).map(|value| value & !mask)
}

fn next_generation(generation: u32) -> u32 {
    let next = generation.wrapping_add(1);
    if next == 0 { 1 } else { next }
}
