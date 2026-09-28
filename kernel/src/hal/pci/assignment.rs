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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Assignment {
    pub index: u8,
    pub kind: bar::Kind,
    pub prefetchable: bool,
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
    #[cfg_attr(not(test), expect(dead_code, reason = "caller-supplied apertures stay explicit until ACPI host bridge resources are available"))]
    pub const fn new(io: resource::Range, mmio32: resource::Range, mmio64: resource::Range) -> Self {
        Self {
            io: resource::Allocator::new(io.base, io.size),
            mmio32: resource::Allocator::new(mmio32.base, mmio32.size),
            mmio64: resource::Allocator::new(mmio64.base, mmio64.size),
        }
    }

    pub fn from_ranges(
        io: &[resource::Range],
        memory: &[resource::Range],
        prefetch: &[resource::Range],
    ) -> Result<Self, resource::Error> {
        Ok(Self {
            io: resource::Allocator::from_ranges(io)?,
            mmio32: resource::Allocator::from_ranges(memory)?,
            mmio64: resource::Allocator::from_ranges(prefetch)?,
        })
    }
}

pub fn release_all(
    apertures: &mut Apertures,
    assignments: &[Option<Assignment>; 6],
) -> Result<(), resource::Error> {
    let mut first_error = None;
    for assignment in assignments.iter().rev().flatten().copied() {
        let allocator = match assignment.kind {
            bar::Kind::Io => &mut apertures.io,
            bar::Kind::Memory32 | bar::Kind::Memory64 if assignment.prefetchable => {
                &mut apertures.mmio64
            }
            bar::Kind::Memory32 | bar::Kind::Memory64 => &mut apertures.mmio32,
        };
        if let Err(error) = allocator.release(assignment.reservation) {
            first_error.get_or_insert(error);
        }
    }
    first_error.map_or(Ok(()), Err)
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
            bar::Kind::Memory32 | bar::Kind::Memory64 if probe.prefetchable => {
                &mut self.apertures.mmio64
            }
            bar::Kind::Memory32 | bar::Kind::Memory64 => &mut self.apertures.mmio32,
        };
        let reservation = allocator.reserve(probe.size, probe.size).map_err(Error::Resource)?;
        let assignment = Assignment {
            index: request.index,
            kind: probe.kind,
            prefetchable: probe.prefetchable,
            reservation,
        };
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
            let allocator = match assignment.kind {
                bar::Kind::Io => &mut self.apertures.io,
                bar::Kind::Memory32 | bar::Kind::Memory64 if assignment.prefetchable => {
                    &mut self.apertures.mmio64
                }
                bar::Kind::Memory32 | bar::Kind::Memory64 => &mut self.apertures.mmio32,
            };
            let _ = allocator.release(assignment.reservation);
        }
    }
}
