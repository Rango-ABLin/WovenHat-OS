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
