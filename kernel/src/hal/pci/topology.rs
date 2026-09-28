//! Bounded PCIe function/bridge topology and ownership lifecycle.
//!
//! Allocation-free state. Function and MMIO handles carry generations so a
//! stale owner cannot address a slot after teardown and reuse.

pub const MAX_FUNCTIONS: usize = 64;
pub const MAX_MMIO_LEASES: usize = 96;
pub const NO_OWNER: u32 = 0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FunctionAddress {
    pub segment: u16,
    pub bus: u8,
    pub device: u8,
    pub function: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FunctionHandle {
    pub slot: u8,
    pub generation: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MmioLease {
    pub slot: u8,
    pub generation: u32,
    pub function: FunctionHandle,
    pub bar: u8,
    pub base: u64,
    pub size: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BridgeRoute {
    pub primary: u8,
    pub secondary: u8,
    pub subordinate: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FunctionDescriptor {
    pub address: FunctionAddress,
    pub bridge: Option<BridgeRoute>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NodeSnapshot {
    pub handle: FunctionHandle,
    pub address: FunctionAddress,
    pub parent: Option<FunctionHandle>,
    pub bridge: Option<BridgeRoute>,
    pub owner: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Capacity,
    Duplicate,
    InvalidHandle,
    InvalidOwner,
    AlreadyOwned,
    NotOwner,
    InvalidBar,
    InvalidBridge,
}

#[derive(Clone, Copy)]
struct Node {
    generation: u32,
    occupied: bool,
    address: FunctionAddress,
    parent: Option<FunctionHandle>,
    bridge: Option<BridgeRoute>,
    owner: u32,
}

impl Node {
    const fn empty() -> Self {
        Self {
            generation: 1,
            occupied: false,
            address: FunctionAddress { segment: 0, bus: 0, device: 0, function: 0 },
            parent: None,
            bridge: None,
            owner: NO_OWNER,
        }
    }
}

#[derive(Clone, Copy)]
struct LeaseSlot {
    generation: u32,
    occupied: bool,
    function: FunctionHandle,
    bar: u8,
    base: u64,
    size: u64,
}

impl LeaseSlot {
    const fn empty() -> Self {
        Self {
            generation: 1,
            occupied: false,
            function: FunctionHandle { slot: 0, generation: 0 },
            bar: 0,
            base: 0,
            size: 0,
        }
    }
}

#[derive(Clone)]
pub struct Topology {
    nodes: [Node; MAX_FUNCTIONS],
    leases: [LeaseSlot; MAX_MMIO_LEASES],
    count: usize,
}

impl Topology {
    pub const fn new() -> Self {
        Self {
            nodes: [Node::empty(); MAX_FUNCTIONS],
            leases: [LeaseSlot::empty(); MAX_MMIO_LEASES],
            count: 0,
        }
    }

    pub fn count(&self) -> usize { self.count }

    pub fn handle_at(&self, slot: usize) -> Option<FunctionHandle> {
        let node = self.nodes.get(slot)?;
        node.occupied.then_some(FunctionHandle { slot: slot as u8, generation: node.generation })
    }

    pub fn insert(&mut self, descriptor: FunctionDescriptor) -> Result<FunctionHandle, Error> {
        if descriptor.address.device >= 32 || descriptor.address.function >= 8 {
            return Err(Error::InvalidHandle);
        }
        if let Some(route) = descriptor.bridge {
            if route.secondary == 0 || route.secondary > route.subordinate {
                return Err(Error::InvalidBridge);
            }
        }
        if self.nodes.iter().any(|node| node.occupied && node.address == descriptor.address) {
            return Err(Error::Duplicate);
        }
        let slot = self.nodes.iter().position(|node| !node.occupied).ok_or(Error::Capacity)?;
        let generation = self.nodes[slot].generation;
        self.nodes[slot] = Node {
            generation,
            occupied: true,
            address: descriptor.address,
            parent: None,
            bridge: descriptor.bridge,
            owner: NO_OWNER,
        };
        self.count += 1;
        let handle = FunctionHandle { slot: slot as u8, generation };
        self.nodes[slot].parent = self.find_parent(descriptor.address, Some(handle));
        self.reparent_children(handle);
        Ok(handle)
    }

    /// Reconcile one discovered function without invalidating an unchanged
    /// generation-safe handle, owner, or MMIO lease. Bridge routing metadata may
    /// change across rescans, so parent relationships are recomputed in place.
    pub fn reconcile(&mut self, descriptor: FunctionDescriptor) -> Result<FunctionHandle, Error> {
        if let Some(index) = self.nodes.iter().position(|node| {
            node.occupied && node.address == descriptor.address
        }) {
            if let Some(route) = descriptor.bridge {
                if route.secondary == 0 || route.secondary > route.subordinate {
                    return Err(Error::InvalidBridge);
                }
            }
            self.nodes[index].bridge = descriptor.bridge;
            let handle = FunctionHandle {
                slot: index as u8,
                generation: self.nodes[index].generation,
            };
            for node_index in 0..MAX_FUNCTIONS {
                if self.nodes[node_index].occupied {
                    let address = self.nodes[node_index].address;
                    let current = FunctionHandle {
                        slot: node_index as u8,
                        generation: self.nodes[node_index].generation,
                    };
                    self.nodes[node_index].parent = self.find_parent(address, Some(current));
                }
            }
            return Ok(handle);
        }
        self.insert(descriptor)
    }

    pub fn snapshot(&self, handle: FunctionHandle) -> Result<NodeSnapshot, Error> {
        let node = self.node(handle)?;
        Ok(NodeSnapshot {
            handle,
            address: node.address,
            parent: node.parent,
            bridge: node.bridge,
            owner: (node.owner != NO_OWNER).then_some(node.owner),
        })
    }

    pub fn claim(&mut self, handle: FunctionHandle, owner: u32) -> Result<(), Error> {
        if owner == NO_OWNER { return Err(Error::InvalidOwner); }
        let node = self.node_mut(handle)?;
        if node.owner != NO_OWNER { return Err(Error::AlreadyOwned); }
        node.owner = owner;
        Ok(())
    }

    pub fn release(&mut self, handle: FunctionHandle, owner: u32) -> Result<(), Error> {
        let node = self.node_mut(handle)?;
        if owner == NO_OWNER || node.owner != owner { return Err(Error::NotOwner); }
        node.owner = NO_OWNER;
        Ok(())
    }

    pub fn lease_mmio(
        &mut self,
        handle: FunctionHandle,
        owner: u32,
        bar: u8,
        base: u64,
        size: u64,
    ) -> Result<MmioLease, Error> {
        if bar >= 6 || base == 0 || size == 0 { return Err(Error::InvalidBar); }
        let node = self.node(handle)?;
        if owner == NO_OWNER || node.owner != owner { return Err(Error::NotOwner); }
        let slot = self.leases.iter().position(|lease| !lease.occupied).ok_or(Error::Capacity)?;
        let generation = self.leases[slot].generation;
        self.leases[slot] = LeaseSlot { generation, occupied: true, function: handle, bar, base, size };
        Ok(MmioLease { slot: slot as u8, generation, function: handle, bar, base, size })
    }

    pub fn validate_mmio(&self, lease: MmioLease, owner: u32) -> bool {
        let Some(slot) = self.leases.get(lease.slot as usize) else { return false; };
        if !slot.occupied || slot.generation != lease.generation || slot.function != lease.function
            || slot.bar != lease.bar || slot.base != lease.base || slot.size != lease.size {
            return false;
        }
        self.node(lease.function)
            .is_ok_and(|node| owner != NO_OWNER && node.owner == owner)
    }

    pub fn release_mmio(&mut self, lease: MmioLease, owner: u32) -> Result<(), Error> {
        if !self.validate_mmio(lease, owner) {
            return Err(Error::NotOwner);
        }
        let slot = &mut self.leases[lease.slot as usize];
        slot.occupied = false;
        slot.generation = next_generation(slot.generation);
        slot.function = FunctionHandle::default();
        slot.bar = 0;
        slot.base = 0;
        slot.size = 0;
        Ok(())
    }

    pub fn teardown(&mut self, handle: FunctionHandle, owner: u32) -> Result<(), Error> {
        let node = self.node(handle)?;
        if node.owner != NO_OWNER && node.owner != owner { return Err(Error::NotOwner); }

        // Invalidate all logical MMIO authority before the function slot can
        // advance generation or be reused.
        for lease in &mut self.leases {
            if lease.occupied && lease.function == handle {
                lease.occupied = false;
                lease.generation = next_generation(lease.generation);
                lease.function = FunctionHandle::default();
                lease.bar = 0;
                lease.base = 0;
                lease.size = 0;
            }
        }

        let slot = handle.slot as usize;
        let generation = next_generation(self.nodes[slot].generation);
        self.nodes[slot] = Node::empty();
        self.nodes[slot].generation = generation;
        self.count -= 1;

        for child in &mut self.nodes {
            if child.occupied && child.parent == Some(handle) { child.parent = None; }
        }
        for index in 0..MAX_FUNCTIONS {
            if self.nodes[index].occupied {
                let address = self.nodes[index].address;
                let current = FunctionHandle {
                    slot: index as u8,
                    generation: self.nodes[index].generation,
                };
                self.nodes[index].parent = self.find_parent(address, Some(current));
            }
        }
        Ok(())
    }

    fn node(&self, handle: FunctionHandle) -> Result<&Node, Error> {
        let node = self.nodes.get(handle.slot as usize).ok_or(Error::InvalidHandle)?;
        if !node.occupied || node.generation != handle.generation { return Err(Error::InvalidHandle); }
        Ok(node)
    }

    fn node_mut(&mut self, handle: FunctionHandle) -> Result<&mut Node, Error> {
        let node = self.nodes.get_mut(handle.slot as usize).ok_or(Error::InvalidHandle)?;
        if !node.occupied || node.generation != handle.generation { return Err(Error::InvalidHandle); }
        Ok(node)
    }

    fn find_parent(
        &self,
        address: FunctionAddress,
        exclude: Option<FunctionHandle>,
    ) -> Option<FunctionHandle> {
        let mut best: Option<(u8, FunctionHandle)> = None;
        for (index, node) in self.nodes.iter().enumerate() {
            if !node.occupied || node.address.segment != address.segment { continue; }
            let handle = FunctionHandle { slot: index as u8, generation: node.generation };
            if exclude == Some(handle) { continue; }
            let Some(route) = node.bridge else { continue; };
            if address.bus < route.secondary || address.bus > route.subordinate { continue; }
            let span = route.subordinate - route.secondary;
            if best.is_none_or(|(best_span, _)| span < best_span) {
                best = Some((span, handle));
            }
        }
        best.map(|(_, handle)| handle)
    }

    fn reparent_children(&mut self, bridge: FunctionHandle) {
        let Ok(node) = self.node(bridge) else { return; };
        let Some(route) = node.bridge else { return; };
        let segment = node.address.segment;
        for index in 0..MAX_FUNCTIONS {
            if !self.nodes[index].occupied || index == bridge.slot as usize { continue; }
            let address = self.nodes[index].address;
            if address.segment != segment
                || address.bus < route.secondary
                || address.bus > route.subordinate {
                continue;
            }
            let current = FunctionHandle {
                slot: index as u8,
                generation: self.nodes[index].generation,
            };
            self.nodes[index].parent = self.find_parent(address, Some(current));
        }
    }
}

const fn next_generation(generation: u32) -> u32 {
    let next = generation.wrapping_add(1);
    if next == 0 { 1 } else { next }
}
