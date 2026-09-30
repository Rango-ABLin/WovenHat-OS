//! Stage 13.10A Bluetooth HCI core.
//!
//! Hardware-independent HCI command/event framing and bounded controller state.
//! USB transport, radio discovery, pairing and encryption are later slices.

pub const HCI_COMMAND_PACKET: u8 = 0x01;
pub const HCI_EVENT_PACKET: u8 = 0x04;
pub const EVT_COMMAND_COMPLETE: u8 = 0x0e;
pub const EVT_COMMAND_STATUS: u8 = 0x0f;
pub const OPCODE_RESET: u16 = 0x0c03;
pub const OPCODE_READ_LOCAL_VERSION: u16 = 0x1001;
pub const OPCODE_READ_LOCAL_SUPPORTED_COMMANDS: u16 = 0x1002;
pub const OPCODE_READ_BD_ADDR: u16 = 0x1009;
pub const MAX_HCI_EVENT: usize = 257;
pub const MAX_HCI_PAYLOAD: usize = 255;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HciError { PayloadTooLarge, MalformedEvent, UnexpectedOpcode, ControllerFailure(u8) }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HciCommand {
    pub opcode: u16,
    pub len: u8,
    payload: [u8; MAX_HCI_PAYLOAD],
}

impl HciCommand {
    pub fn new(opcode: u16, data: &[u8]) -> Result<Self, HciError> {
        if data.len() > MAX_HCI_PAYLOAD { return Err(HciError::PayloadTooLarge); }
        let mut payload = [0; MAX_HCI_PAYLOAD];
        payload[..data.len()].copy_from_slice(data);
        Ok(Self { opcode, len: data.len() as u8, payload })
    }
    pub fn payload(&self) -> &[u8] { &self.payload[..self.len as usize] }
    pub fn encoded_len(&self) -> usize { 3 + self.len as usize }
    pub fn encode(&self, out: &mut [u8; 258]) -> usize {
        out[0..2].copy_from_slice(&self.opcode.to_le_bytes());
        out[2] = self.len;
        let n = self.len as usize;
        out[3..3+n].copy_from_slice(self.payload());
        3+n
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandComplete { pub credits: u8, pub opcode: u16, pub status: u8 }

pub fn parse_command_complete(event: &[u8]) -> Result<CommandComplete, HciError> {
    if event.len() < 6 || event[0] != EVT_COMMAND_COMPLETE || event[1] < 4 {
        return Err(HciError::MalformedEvent);
    }
    let opcode = u16::from_le_bytes([event[3], event[4]]);
    let status = event[5];
    Ok(CommandComplete { credits: event[2], opcode, status })
}

pub struct ControllerState { credits: u8, last_opcode: Option<u16> }

impl ControllerState {
    pub const fn new() -> Self { Self { credits: 1, last_opcode: None } }
    pub fn begin(&mut self, command: &HciCommand) -> bool {
        if self.credits == 0 { return false; }
        self.credits -= 1;
        self.last_opcode = Some(command.opcode);
        true
    }
    pub fn complete(&mut self, event: &[u8]) -> Result<(), HciError> {
        let done = parse_command_complete(event)?;
        if self.last_opcode != Some(done.opcode) { return Err(HciError::UnexpectedOpcode); }
        self.credits = done.credits;
        self.last_opcode = None;
        if done.status != 0 { return Err(HciError::ControllerFailure(done.status)); }
        Ok(())
    }
    pub fn ready(&self) -> bool { self.credits != 0 && self.last_opcode.is_none() }
}

pub struct HciTransaction {
    state: ControllerState,
}

impl HciTransaction {
    pub const fn new() -> Self {
        Self {
            state: ControllerState::new(),
        }
    }

    pub fn begin_command(
        &mut self,
        command: &HciCommand,
        out: &mut [u8; 258],
    ) -> Result<usize, HciError> {
        if !self.state.begin(command) {
            return Err(HciError::ControllerFailure(0xff));
        }
        Ok(command.encode(out))
    }

    pub fn complete_event(&mut self, event: &[u8]) -> Result<(), HciError> {
        self.state.complete(event)
    }

    pub fn ready(&self) -> bool {
        self.state.ready()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InitializationStep {
    Reset,
    ReadLocalVersion,
    Ready,
}

pub struct ControllerInitializer {
    transaction: HciTransaction,
    step: InitializationStep,
}

impl ControllerInitializer {
    pub const fn new() -> Self {
        Self {
            transaction: HciTransaction::new(),
            step: InitializationStep::Reset,
        }
    }

    pub fn step(&self) -> InitializationStep {
        self.step
    }

    pub fn next_command(&mut self, out: &mut [u8; 258]) -> Result<Option<usize>, HciError> {
        let opcode = match self.step {
            InitializationStep::Reset => OPCODE_RESET,
            InitializationStep::ReadLocalVersion => OPCODE_READ_LOCAL_VERSION,
            InitializationStep::Ready => return Ok(None),
        };
        let command = HciCommand::new(opcode, &[])?;
        self.transaction.begin_command(&command, out).map(Some)
    }

    pub fn complete(&mut self, event: &[u8]) -> Result<(), HciError> {
        self.transaction.complete_event(event)?;
        self.step = match self.step {
            InitializationStep::Reset => InitializationStep::ReadLocalVersion,
            InitializationStep::ReadLocalVersion => InitializationStep::Ready,
            InitializationStep::Ready => InitializationStep::Ready,
        };
        Ok(())
    }

    pub fn ready(&self) -> bool {
        self.step == InitializationStep::Ready && self.transaction.ready()
    }
}

pub fn initialization_self_test() -> bool {
    let mut init = ControllerInitializer::new();
    let mut bytes = [0_u8; 258];
    if init.step() != InitializationStep::Reset
        || init.next_command(&mut bytes) != Ok(Some(3))
        || bytes[..3] != [0x03, 0x0c, 0]
    {
        return false;
    }
    if init
        .complete(&[EVT_COMMAND_COMPLETE, 4, 1, 0x03, 0x0c, 0])
        .is_err()
        || init.step() != InitializationStep::ReadLocalVersion
    {
        return false;
    }
    if init.next_command(&mut bytes) != Ok(Some(3)) || bytes[..3] != [0x01, 0x10, 0] {
        return false;
    }
    init.complete(&[EVT_COMMAND_COMPLETE, 4, 1, 0x01, 0x10, 0]).is_ok()
        && init.ready()
        && init.next_command(&mut bytes) == Ok(None)
}



#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalVersion {
    pub hci_version: u8,
    pub hci_revision: u16,
    pub lmp_version: u8,
    pub manufacturer: u16,
    pub lmp_subversion: u16,
}

pub fn parse_local_version_complete(event: &[u8]) -> Result<LocalVersion, HciError> {
    let done = parse_command_complete(event)?;
    if done.opcode != OPCODE_READ_LOCAL_VERSION {
        return Err(HciError::UnexpectedOpcode);
    }
    if done.status != 0 {
        return Err(HciError::ControllerFailure(done.status));
    }
    if event.len() < 14 || event[1] < 12 {
        return Err(HciError::MalformedEvent);
    }
    Ok(LocalVersion {
        hci_version: event[6],
        hci_revision: u16::from_le_bytes([event[7], event[8]]),
        lmp_version: event[9],
        manufacturer: u16::from_le_bytes([event[10], event[11]]),
        lmp_subversion: u16::from_le_bytes([event[12], event[13]]),
    })
}

pub fn parse_bd_addr_complete(event: &[u8]) -> Result<[u8; 6], HciError> {
    let done = parse_command_complete(event)?;
    if done.opcode != OPCODE_READ_BD_ADDR {
        return Err(HciError::UnexpectedOpcode);
    }
    if done.status != 0 {
        return Err(HciError::ControllerFailure(done.status));
    }
    if event.len() < 12 || event[1] < 10 {
        return Err(HciError::MalformedEvent);
    }
    let mut address = [0_u8; 6];
    address.copy_from_slice(&event[6..12]);
    Ok(address)
}

pub fn parse_supported_commands_complete(event: &[u8]) -> Result<[u8; 64], HciError> {
    let done = parse_command_complete(event)?;
    if done.opcode != OPCODE_READ_LOCAL_SUPPORTED_COMMANDS {
        return Err(HciError::UnexpectedOpcode);
    }
    if done.status != 0 {
        return Err(HciError::ControllerFailure(done.status));
    }
    if event.len() < 70 || event[1] < 68 {
        return Err(HciError::MalformedEvent);
    }
    let mut commands = [0_u8; 64];
    commands.copy_from_slice(&event[6..70]);
    Ok(commands)
}

pub fn capability_self_test() -> bool {
    let version = [
        EVT_COMMAND_COMPLETE, 12, 1, 0x01, 0x10, 0,
        0x0c, 0x34, 0x12, 0x0c, 0x4c, 0x00, 0x78, 0x56,
    ];
    let Ok(info) = parse_local_version_complete(&version) else { return false; };
    if info.hci_version != 0x0c
        || info.hci_revision != 0x1234
        || info.lmp_version != 0x0c
        || info.manufacturer != 0x004c
        || info.lmp_subversion != 0x5678
    {
        return false;
    }

    let address_event = [
        EVT_COMMAND_COMPLETE, 10, 1, 0x09, 0x10, 0,
        1, 2, 3, 4, 5, 6,
    ];
    if parse_bd_addr_complete(&address_event) != Ok([1, 2, 3, 4, 5, 6]) {
        return false;
    }

    let mut supported = [0_u8; 70];
    supported[0] = EVT_COMMAND_COMPLETE;
    supported[1] = 68;
    supported[2] = 1;
    supported[3..5].copy_from_slice(&OPCODE_READ_LOCAL_SUPPORTED_COMMANDS.to_le_bytes());
    supported[5] = 0;
    for (index, byte) in supported[6..70].iter_mut().enumerate() {
        *byte = index as u8;
    }
    let Ok(commands) = parse_supported_commands_complete(&supported) else { return false; };
    commands[0] == 0
        && commands[63] == 63
        && parse_bd_addr_complete(&version).is_err()
        && parse_supported_commands_complete(&supported[..69]).is_err()
}

pub const EVT_INQUIRY_COMPLETE: u8 = 0x01;
pub const EVT_INQUIRY_RESULT: u8 = 0x02;
pub const OPCODE_INQUIRY: u16 = 0x0401;
pub const INQUIRY_GIAC: [u8; 3] = [0x33, 0x8b, 0x9e];
pub const MAX_DISCOVERED_DEVICES: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiscoveredDevice {
    pub address: [u8; 6],
    pub page_scan_repetition_mode: u8,
    pub class_of_device: [u8; 3],
    pub clock_offset: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiscoveryEvent {
    Results(usize),
    Complete,
}

pub struct DiscoveryState {
    devices: [Option<DiscoveredDevice>; MAX_DISCOVERED_DEVICES],
    count: usize,
    active: bool,
}

impl DiscoveryState {
    pub const fn new() -> Self {
        Self {
            devices: [None; MAX_DISCOVERED_DEVICES],
            count: 0,
            active: false,
        }
    }

    pub fn begin(&mut self, out: &mut [u8; 258]) -> Result<usize, HciError> {
        let params = [INQUIRY_GIAC[0], INQUIRY_GIAC[1], INQUIRY_GIAC[2], 0x08, 0x00];
        let command = HciCommand::new(OPCODE_INQUIRY, &params)?;
        self.active = true;
        Ok(command.encode(out))
    }

    pub fn handle_event(&mut self, event: &[u8]) -> Result<DiscoveryEvent, HciError> {
        if event.len() < 2 {
            return Err(HciError::MalformedEvent);
        }
        let parameter_len = event[1] as usize;
        if event.len() < 2 + parameter_len {
            return Err(HciError::MalformedEvent);
        }
        match event[0] {
            EVT_INQUIRY_COMPLETE => {
                if parameter_len != 1 {
                    return Err(HciError::MalformedEvent);
                }
                self.active = false;
                if event[2] != 0 {
                    return Err(HciError::ControllerFailure(event[2]));
                }
                Ok(DiscoveryEvent::Complete)
            }
            EVT_INQUIRY_RESULT => {
                if parameter_len < 1 {
                    return Err(HciError::MalformedEvent);
                }
                let responses = event[2] as usize;
                if parameter_len != 1 + responses * 14 {
                    return Err(HciError::MalformedEvent);
                }
                let mut added = 0;
                for index in 0..responses {
                    let base = 3 + index * 14;
                    let mut address = [0_u8; 6];
                    address.copy_from_slice(&event[base..base + 6]);
                    if self.devices[..self.count]
                        .iter()
                        .flatten()
                        .any(|device| device.address == address)
                    {
                        continue;
                    }
                    if self.count == MAX_DISCOVERED_DEVICES {
                        continue;
                    }
                    let device = DiscoveredDevice {
                        address,
                        page_scan_repetition_mode: event[base + 6],
                        class_of_device: [event[base + 9], event[base + 10], event[base + 11]],
                        clock_offset: u16::from_le_bytes([event[base + 12], event[base + 13]]),
                    };
                    self.devices[self.count] = Some(device);
                    self.count += 1;
                    added += 1;
                }
                Ok(DiscoveryEvent::Results(added))
            }
            _ => Err(HciError::MalformedEvent),
        }
    }

    pub fn active(&self) -> bool { self.active }
    pub fn count(&self) -> usize { self.count }
    pub fn device(&self, index: usize) -> Option<DiscoveredDevice> {
        if index >= self.count { None } else { self.devices[index] }
    }
}

pub fn discovery_self_test() -> bool {
    let mut discovery = DiscoveryState::new();
    let mut bytes = [0_u8; 258];
    if discovery.begin(&mut bytes) != Ok(8)
        || bytes[..8] != [0x01, 0x04, 0x05, 0x33, 0x8b, 0x9e, 0x08, 0x00]
        || !discovery.active()
    {
        return false;
    }

    let result = [
        EVT_INQUIRY_RESULT, 15, 1,
        1, 2, 3, 4, 5, 6,
        1, 0, 0,
        0x04, 0x02, 0x0c,
        0x34, 0x12,
    ];
    if discovery.handle_event(&result) != Ok(DiscoveryEvent::Results(1))
        || discovery.count() != 1
        || discovery.handle_event(&result) != Ok(DiscoveryEvent::Results(0))
        || discovery.count() != 1
    {
        return false;
    }
    let Some(device) = discovery.device(0) else { return false; };
    if device.address != [1, 2, 3, 4, 5, 6]
        || device.page_scan_repetition_mode != 1
        || device.class_of_device != [0x04, 0x02, 0x0c]
        || device.clock_offset != 0x1234
    {
        return false;
    }

    discovery.handle_event(&[EVT_INQUIRY_COMPLETE, 1, 0]) == Ok(DiscoveryEvent::Complete)
        && !discovery.active()
        && discovery.handle_event(&[EVT_INQUIRY_RESULT, 1, 1]).is_err()
}


pub const EVT_CONNECTION_COMPLETE: u8 = 0x03;
pub const EVT_DISCONNECTION_COMPLETE: u8 = 0x05;
pub const OPCODE_CREATE_CONNECTION: u16 = 0x0405;
pub const OPCODE_DISCONNECT: u16 = 0x0406;
pub const MAX_ACL_LINKS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AclLink {
    pub handle: u16,
    pub address: [u8; 6],
    pub encrypted: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkEvent {
    Connected(AclLink),
    Disconnected(u16),
}

pub struct LinkState {
    links: [Option<AclLink>; MAX_ACL_LINKS],
    count: usize,
}

impl LinkState {
    pub const fn new() -> Self {
        Self { links: [None; MAX_ACL_LINKS], count: 0 }
    }

    pub fn create_connection_command(
        device: DiscoveredDevice,
        out: &mut [u8; 258],
    ) -> Result<usize, HciError> {
        let mut params = [0_u8; 13];
        params[0..6].copy_from_slice(&device.address);
        params[6..8].copy_from_slice(&0xcc18_u16.to_le_bytes());
        params[8] = device.page_scan_repetition_mode;
        params[9] = 0;
        params[10..12].copy_from_slice(&device.clock_offset.to_le_bytes());
        params[12] = 1;
        HciCommand::new(OPCODE_CREATE_CONNECTION, &params).map(|command| command.encode(out))
    }

    pub fn disconnect_command(handle: u16, reason: u8, out: &mut [u8; 258]) -> Result<usize, HciError> {
        if handle > 0x0fff {
            return Err(HciError::MalformedEvent);
        }
        let [lo, hi] = handle.to_le_bytes();
        HciCommand::new(OPCODE_DISCONNECT, &[lo, hi, reason]).map(|command| command.encode(out))
    }

    pub fn handle_event(&mut self, event: &[u8]) -> Result<LinkEvent, HciError> {
        if event.len() < 2 || event.len() < 2 + event[1] as usize {
            return Err(HciError::MalformedEvent);
        }
        match event[0] {
            EVT_CONNECTION_COMPLETE => {
                if event[1] != 11 {
                    return Err(HciError::MalformedEvent);
                }
                if event[2] != 0 {
                    return Err(HciError::ControllerFailure(event[2]));
                }
                let handle = u16::from_le_bytes([event[3], event[4]]);
                if handle > 0x0fff {
                    return Err(HciError::MalformedEvent);
                }
                let mut address = [0_u8; 6];
                address.copy_from_slice(&event[5..11]);
                if event[11] != 1 {
                    return Err(HciError::MalformedEvent);
                }
                if let Some(existing) = self.links[..self.count]
                    .iter()
                    .flatten()
                    .find(|link| link.handle == handle || link.address == address)
                {
                    if existing.handle != handle || existing.address != address {
                        return Err(HciError::UnexpectedOpcode);
                    }
                    return Ok(LinkEvent::Connected(*existing));
                }
                if self.count == MAX_ACL_LINKS {
                    return Err(HciError::ControllerFailure(0xff));
                }
                let link = AclLink { handle, address, encrypted: event[12] != 0 };
                self.links[self.count] = Some(link);
                self.count += 1;
                Ok(LinkEvent::Connected(link))
            }
            EVT_DISCONNECTION_COMPLETE => {
                if event[1] != 4 {
                    return Err(HciError::MalformedEvent);
                }
                if event[2] != 0 {
                    return Err(HciError::ControllerFailure(event[2]));
                }
                let handle = u16::from_le_bytes([event[3], event[4]]);
                let Some(index) = self.links[..self.count]
                    .iter()
                    .position(|link| link.is_some_and(|link| link.handle == handle))
                else {
                    return Err(HciError::UnexpectedOpcode);
                };
                self.count -= 1;
                self.links[index] = self.links[self.count];
                self.links[self.count] = None;
                Ok(LinkEvent::Disconnected(handle))
            }
            _ => Err(HciError::MalformedEvent),
        }
    }

    pub fn count(&self) -> usize { self.count }
    pub fn link(&self, index: usize) -> Option<AclLink> {
        if index >= self.count { None } else { self.links[index] }
    }
}


pub const HCI_ACL_PACKET: u8 = 0x02;
pub const MAX_ACL_PAYLOAD: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AclPacket {
    pub handle: u16,
    pub packet_boundary: u8,
    pub broadcast: u8,
    pub len: u16,
    payload: [u8; MAX_ACL_PAYLOAD],
}

impl AclPacket {
    pub fn new(handle: u16, packet_boundary: u8, broadcast: u8, data: &[u8]) -> Result<Self, HciError> {
        if handle > 0x0fff || packet_boundary > 0x03 || broadcast > 0x03 || data.len() > MAX_ACL_PAYLOAD {
            return Err(HciError::PayloadTooLarge);
        }
        let mut payload = [0_u8; MAX_ACL_PAYLOAD];
        payload[..data.len()].copy_from_slice(data);
        Ok(Self { handle, packet_boundary, broadcast, len: data.len() as u16, payload })
    }

    pub fn payload(&self) -> &[u8] { &self.payload[..self.len as usize] }

    pub fn encode(&self, out: &mut [u8; MAX_ACL_PAYLOAD + 4]) -> usize {
        let handle_flags = self.handle
            | ((self.packet_boundary as u16) << 12)
            | ((self.broadcast as u16) << 14);
        out[0..2].copy_from_slice(&handle_flags.to_le_bytes());
        out[2..4].copy_from_slice(&self.len.to_le_bytes());
        let n = self.len as usize;
        out[4..4 + n].copy_from_slice(self.payload());
        4 + n
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, HciError> {
        if bytes.len() < 4 {
            return Err(HciError::MalformedEvent);
        }
        let handle_flags = u16::from_le_bytes([bytes[0], bytes[1]]);
        let len = u16::from_le_bytes([bytes[2], bytes[3]]) as usize;
        if len > MAX_ACL_PAYLOAD || bytes.len() < 4 + len {
            return Err(HciError::MalformedEvent);
        }
        Self::new(
            handle_flags & 0x0fff,
            ((handle_flags >> 12) & 0x03) as u8,
            ((handle_flags >> 14) & 0x03) as u8,
            &bytes[4..4 + len],
        )
    }
}

impl LinkState {
    pub fn owns_handle(&self, handle: u16) -> bool {
        self.links[..self.count]
            .iter()
            .flatten()
            .any(|link| link.handle == handle)
    }

    pub fn outbound_acl(&self, handle: u16, data: &[u8]) -> Result<AclPacket, HciError> {
        if !self.owns_handle(handle) {
            return Err(HciError::UnexpectedOpcode);
        }
        AclPacket::new(handle, 0x02, 0, data)
    }

    pub fn inbound_acl(&self, bytes: &[u8]) -> Result<AclPacket, HciError> {
        let packet = AclPacket::parse(bytes)?;
        if !self.owns_handle(packet.handle) {
            return Err(HciError::UnexpectedOpcode);
        }
        Ok(packet)
    }
}

pub fn acl_data_self_test() -> bool {
    let mut links = LinkState::new();
    let connected = [
        EVT_CONNECTION_COMPLETE, 11, 0, 0x42, 0x00,
        1, 2, 3, 4, 5, 6, 1, 0,
    ];
    if links.handle_event(&connected).is_err() {
        return false;
    }
    let Ok(packet) = links.outbound_acl(0x42, &[0xaa, 0xbb, 0xcc]) else { return false; };
    let mut encoded = [0_u8; MAX_ACL_PAYLOAD + 4];
    if packet.encode(&mut encoded) != 7
        || encoded[..7] != [0x42, 0x20, 3, 0, 0xaa, 0xbb, 0xcc]
    {
        return false;
    }
    let Ok(parsed) = links.inbound_acl(&encoded[..7]) else { return false; };
    parsed.handle == 0x42
        && parsed.packet_boundary == 0x02
        && parsed.broadcast == 0
        && parsed.payload() == [0xaa, 0xbb, 0xcc]
        && links.outbound_acl(0x43, &[1]).is_err()
        && links.inbound_acl(&[0x43, 0x20, 1, 0, 1]).is_err()
        && AclPacket::parse(&[0x42, 0x20, 4, 0, 1]).is_err()
}

pub fn link_lifecycle_self_test() -> bool {
    let device = DiscoveredDevice {
        address: [1, 2, 3, 4, 5, 6],
        page_scan_repetition_mode: 1,
        class_of_device: [0x04, 0x02, 0x0c],
        clock_offset: 0x1234,
    };
    let mut bytes = [0_u8; 258];
    if LinkState::create_connection_command(device, &mut bytes) != Ok(16)
        || bytes[..3] != [0x05, 0x04, 13]
        || bytes[3..9] != device.address
    {
        return false;
    }

    let mut links = LinkState::new();
    let connected = [
        EVT_CONNECTION_COMPLETE, 11, 0, 0x42, 0x00,
        1, 2, 3, 4, 5, 6,
        1, 0,
    ];
    let Ok(LinkEvent::Connected(link)) = links.handle_event(&connected) else { return false; };
    if link.handle != 0x42 || link.address != device.address || link.encrypted || links.count() != 1 {
        return false;
    }
    let conflicting_handle = [
        EVT_CONNECTION_COMPLETE, 11, 0, 0x42, 0x00,
        6, 5, 4, 3, 2, 1,
        1, 0,
    ];
    let conflicting_address = [
        EVT_CONNECTION_COMPLETE, 11, 0, 0x43, 0x00,
        1, 2, 3, 4, 5, 6,
        1, 0,
    ];
    if links.handle_event(&conflicting_handle).is_ok()
        || links.handle_event(&conflicting_address).is_ok()
        || links.count() != 1
    {
        return false;
    }
    if LinkState::disconnect_command(link.handle, 0x13, &mut bytes) != Ok(6)
        || bytes[..6] != [0x06, 0x04, 3, 0x42, 0, 0x13]
    {
        return false;
    }
    links.handle_event(&[EVT_DISCONNECTION_COMPLETE, 4, 0, 0x42, 0, 0x13])
        == Ok(LinkEvent::Disconnected(0x42))
        && links.count() == 0
        && links.link(0).is_none()
        && links.outbound_acl(0x42, &[1]).is_err()
        && links.inbound_acl(&[0x42, 0x20, 1, 0, 1]).is_err()
        && links.handle_event(&connected[..12]).is_err()
        && LinkState::disconnect_command(0x1000, 0x13, &mut bytes).is_err()
}

pub fn transaction_self_test() -> bool {
    let reset = HciCommand::new(OPCODE_RESET, &[]).unwrap();
    let version = HciCommand::new(OPCODE_READ_LOCAL_VERSION, &[]).unwrap();
    let mut tx = HciTransaction::new();
    let mut bytes = [0_u8; 258];

    if tx.begin_command(&reset, &mut bytes) != Ok(3) || bytes[..3] != [0x03, 0x0c, 0] {
        return false;
    }
    if tx.begin_command(&version, &mut bytes).is_ok() {
        return false;
    }
    if tx
        .complete_event(&[EVT_COMMAND_COMPLETE, 4, 1, 0x03, 0x0c, 0])
        .is_err()
        || !tx.ready()
    {
        return false;
    }
    tx.begin_command(&version, &mut bytes) == Ok(3)
        && bytes[..3] == [0x01, 0x10, 0]
        && tx
            .complete_event(&[EVT_COMMAND_COMPLETE, 4, 1, 0x01, 0x10, 0])
            .is_ok()
        && tx.ready()
}

pub fn self_test() -> bool {
    let reset = HciCommand::new(OPCODE_RESET, &[]).unwrap();
    let mut encoded = [0u8; 258];
    let n = reset.encode(&mut encoded);
    if n != 3 || encoded[..3] != [0x03, 0x0c, 0x00] { return false; }

    let mut state = ControllerState::new();
    if !state.begin(&reset) || state.begin(&reset) { return false; }
    if state.complete(&[EVT_COMMAND_COMPLETE, 4, 1, 0x03, 0x0c, 0]).is_err() || !state.ready() { return false; }

    parse_command_complete(&[EVT_COMMAND_COMPLETE, 3, 1, 3, 12]).is_err()
        && HciCommand::new(0x0001, &[0u8; MAX_HCI_PAYLOAD]).is_ok()
}
