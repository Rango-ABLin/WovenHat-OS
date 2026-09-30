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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
