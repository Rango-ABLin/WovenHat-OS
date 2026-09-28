//! Pure bounded PCI bridge-routing planner.
//!
//! The planner has no config-space side effects. Callers feed endpoint BAR
//! ranges and bridge parentage, then consume bridge windows bottom-up.

use super::bridge::{Window, Windows};

pub const MAX_ROUTES: usize = 64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Demand {
    pub io: Option<Window>,
    pub memory: Option<Window>,
    pub prefetch: Option<Window>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bridge {
    pub id: u8,
    pub parent: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Capacity,
    DuplicateBridge,
    MissingBridge,
    InvalidParent,
    Overflow,
    AddressWidth,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Node {
    occupied: bool,
    bridge: Bridge,
    direct: Demand,
    aggregate: Demand,
}

pub struct Plan {
    nodes: [Node; MAX_ROUTES],
    count: usize,
}

impl Plan {
    pub const fn new() -> Self {
        Self { nodes: [Node { occupied: false, bridge: Bridge { id: 0, parent: None }, direct: Demand { io: None, memory: None, prefetch: None }, aggregate: Demand { io: None, memory: None, prefetch: None } }; MAX_ROUTES], count: 0 }
    }

    pub fn add_bridge(&mut self, bridge: Bridge) -> Result<(), Error> {
        if self.find(bridge.id).is_some() { return Err(Error::DuplicateBridge); }
        if bridge.parent == Some(bridge.id) { return Err(Error::InvalidParent); }
        let slot = self.nodes.iter().position(|node| !node.occupied).ok_or(Error::Capacity)?;
        self.nodes[slot] = Node { occupied: true, bridge, direct: Demand::default(), aggregate: Demand::default() };
        self.count += 1;
        Ok(())
    }

    pub fn add_demand(&mut self, bridge_id: u8, demand: Demand) -> Result<(), Error> {
        let index = self.find(bridge_id).ok_or(Error::MissingBridge)?;
        merge_demand(&mut self.nodes[index].direct, demand)
    }

    pub fn solve(&mut self) -> Result<(), Error> {
        for node in &mut self.nodes {
            if node.occupied { node.aggregate = node.direct; }
        }
        for index in 0..MAX_ROUTES {
            if !self.nodes[index].occupied { continue; }
            let mut seen = [false; MAX_ROUTES];
            let mut current = index;
            loop {
                if seen[current] { return Err(Error::InvalidParent); }
                seen[current] = true;
                let Some(parent_id) = self.nodes[current].bridge.parent else { break; };
                current = self.find(parent_id).ok_or(Error::InvalidParent)?;
            }
        }

        // Repeatedly propagate descendants until a fixed point. Capacity is
        // bounded, so this remains deterministic without heap allocation.
        for _ in 0..self.count {
            let previous = self.nodes;
            for index in 0..MAX_ROUTES {
                if !previous[index].occupied { continue; }
                let mut aggregate = previous[index].direct;
                for child in previous.iter().filter(|node| node.occupied && node.bridge.parent == Some(previous[index].bridge.id)) {
                    merge_demand(&mut aggregate, child.aggregate)?;
                }
                self.nodes[index].aggregate = aggregate;
            }
        }
        Ok(())
    }

    pub fn windows(&self, bridge_id: u8) -> Result<Windows, Error> {
        let node = self.nodes[self.find(bridge_id).ok_or(Error::MissingBridge)?];
        Ok(Windows { io: node.aggregate.io, memory: node.aggregate.memory, prefetch: node.aggregate.prefetch })
    }

    pub fn depth(&self, bridge_id: u8) -> Result<u8, Error> {
        let mut current = self.find(bridge_id).ok_or(Error::MissingBridge)?;
        let mut depth = 0u8;
        let mut seen = [false; MAX_ROUTES];
        loop {
            if seen[current] { return Err(Error::InvalidParent); }
            seen[current] = true;
            let Some(parent_id) = self.nodes[current].bridge.parent else { return Ok(depth); };
            current = self.find(parent_id).ok_or(Error::InvalidParent)?;
            depth = depth.checked_add(1).ok_or(Error::Overflow)?;
        }
    }

    fn find(&self, id: u8) -> Option<usize> {
        self.nodes.iter().position(|node| node.occupied && node.bridge.id == id)
    }
}

fn merge_demand(target: &mut Demand, incoming: Demand) -> Result<(), Error> {
    merge_window(&mut target.io, incoming.io, 0x1000, true)?;
    merge_window(&mut target.memory, incoming.memory, 0x10_0000, false)?;
    merge_window(&mut target.prefetch, incoming.prefetch, 0x10_0000, false)?;
    Ok(())
}

fn merge_window(target: &mut Option<Window>, incoming: Option<Window>, granularity: u64, io: bool) -> Result<(), Error> {
    let Some(incoming) = incoming else { return Ok(()); };
    let incoming_end = incoming.base.checked_add(incoming.size).ok_or(Error::Overflow)?;
    let mut base = incoming.base & !(granularity - 1);
    let mut end = incoming_end.checked_add(granularity - 1).ok_or(Error::Overflow)? & !(granularity - 1);
    if let Some(current) = *target {
        let current_end = current.base.checked_add(current.size).ok_or(Error::Overflow)?;
        base = core::cmp::min(base, current.base);
        end = core::cmp::max(end, current_end);
    }
    if io && end > u64::from(u32::MAX) + 1 { return Err(Error::AddressWidth); }
    if !io && target as *const _ == target as *const _ {
        // Width is validated by bridge encoders; prefetchable windows may be 64-bit.
    }
    *target = Some(Window { base, size: end.checked_sub(base).ok_or(Error::Overflow)? });
    Ok(())
}
