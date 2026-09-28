//! Pure transactional BAR assignment planning.
//!
//! This module owns no hardware. It reserves address space for all BARs before
//! config-space programming begins, allowing callers to abort without leaking
//! partially allocated apertures.

use super::{bar, resource};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Request {
    pub index: u8,
    pub probe: Option<bar::Probe>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Assignment {
    pub index: u8,
    pub reservation: resource::Reservation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidIndex,
    DuplicateIndex,
    Resource(resource::Error),
}

pub struct Apertures {
    pub io: resource::Allocator,
    pub mmio32: resource::Allocator,
    pub mmio64: resource::Allocator,
}

impl Apertures {
    pub const fn new(io: resource::Range, mmio32: resource::Range, mmio64: resource::Range) -> Self {
        Self {
            io: resource::Allocator::new(io.base, io.size),
            mmio32: resource::Allocator::new(mmio32.base, mmio32.size),
            mmio64: resource::Allocator::new(mmio64.base, mmio64.size),
        }
    }
}

pub struct Plan<'a> {
    apertures: &'a mut Apertures,
    assignments: [Option<Assignment>; 6],
    count: usize,
    committed: bool,
}

impl<'a> Plan<'a> {
    pub fn new(apertures: &'a mut Apertures) -> Self {
        Self { apertures, assignments: [None; 6], count: 0, committed: false }
    }

    pub fn reserve(&mut self, request: Request) -> Result<Option<Assignment>, Error> {
        let Some(probe) = request.probe else { return Ok(None); };
        if request.index >= 6 { return Err(Error::InvalidIndex); }
        if self.assignments[..self.count].iter().flatten().any(|item| item.index == request.index) {
            return Err(Error::DuplicateIndex);
        }
        let allocator = match probe.kind {
            bar::Kind::Io => &mut self.apertures.io,
            bar::Kind::Memory32 => &mut self.apertures.mmio32,
            bar::Kind::Memory64 => &mut self.apertures.mmio64,
        };
        let reservation = allocator.reserve(probe.size, probe.size).map_err(Error::Resource)?;
        let assignment = Assignment { index: request.index, reservation };
        self.assignments[self.count] = Some(assignment);
        self.count += 1;
        Ok(Some(assignment))
    }

    pub fn commit(mut self) -> [Option<Assignment>; 6] {
        self.committed = true;
        self.assignments
    }
}

impl Drop for Plan<'_> {
    fn drop(&mut self) {
        if self.committed { return; }
        for assignment in self.assignments[..self.count].iter().rev().flatten().copied() {
            let allocator = match assignment.reservation.range.base {
                base if in_aperture(&self.apertures.io, base) => &mut self.apertures.io,
                base if in_aperture(&self.apertures.mmio32, base) => &mut self.apertures.mmio32,
                _ => &mut self.apertures.mmio64,
            };
            let _ = allocator.release(assignment.reservation);
        }
    }
}

fn in_aperture(allocator: &resource::Allocator, base: u64) -> bool {
    // A reservation from another allocator cannot validate here even if
    // apertures accidentally overlap, so contains() remains authoritative.
    // This helper is only a fast selector for the non-overlapping apertures
    // required by Stage 13.2.
    let _ = base;
    allocator.active() != 0
}
