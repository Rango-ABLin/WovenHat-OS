//! Bounded pseudo-terminal pairs with a small canonical line discipline.
//!
//! PTYs are intentionally fixed-capacity kernel objects.  A master write is
//! input to the slave; a slave write is output readable from the master.  The
//! canonical buffer is private to the pair and is never exposed as a pointer
//! retained by the kernel.

use crate::irq_lock::PreemptMutex as Mutex;

const MAX_PTYS: usize = 8;
const BUFFER_SIZE: usize = 4096;
const LINE_SIZE: usize = 512;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PtyId {
    pub index: usize,
    epoch: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Full,
    Invalid,
    Closed,
}

#[derive(Clone, Copy)]
struct Buffer {
    data: [u8; BUFFER_SIZE],
    head: usize,
    tail: usize,
    len: usize,
}

impl Buffer {
    const fn empty() -> Self {
        Self {
            data: [0; BUFFER_SIZE],
            head: 0,
            tail: 0,
            len: 0,
        }
    }

    fn push(&mut self, byte: u8) -> Result<(), Error> {
        if self.len == BUFFER_SIZE {
            return Err(Error::Full);
        }
        self.data[self.tail] = byte;
        self.tail = (self.tail + 1) % BUFFER_SIZE;
        self.len += 1;
        Ok(())
    }

    fn pop(&mut self) -> Option<u8> {
        if self.len == 0 {
            return None;
        }
        let byte = self.data[self.head];
        self.head = (self.head + 1) % BUFFER_SIZE;
        self.len -= 1;
        Some(byte)
    }
}

struct Pty {
    epoch: u64,
    occupied: bool,
    master_refs: u16,
    slave_refs: u16,
    input: Buffer,
    output: Buffer,
    line: [u8; LINE_SIZE],
    line_len: usize,
    eof: bool,
}

impl Pty {
    const fn empty() -> Self {
        Self {
            epoch: 0,
            occupied: false,
            master_refs: 0,
            slave_refs: 0,
            input: Buffer::empty(),
            output: Buffer::empty(),
            line: [0; LINE_SIZE],
            line_len: 0,
            eof: false,
        }
    }
}

struct Table {
    ptys: [Pty; MAX_PTYS],
}

impl Table {
    const fn empty() -> Self {
        Self {
            ptys: [const { Pty::empty() }; MAX_PTYS],
        }
    }

    fn get_mut(&mut self, id: PtyId) -> Result<&mut Pty, Error> {
        self.ptys
            .get_mut(id.index)
            .filter(|pty| pty.occupied && pty.epoch == id.epoch)
            .ok_or(Error::Invalid)
    }
}

static TABLE: Mutex<Table> = Mutex::with_rank(Table::empty(), 10);

pub fn create() -> Result<PtyId, Error> {
    let mut table = TABLE.lock();
    let index = table
        .ptys
        .iter()
        .position(|pty| !pty.occupied && pty.epoch < u64::MAX)
        .ok_or(Error::Full)?;
    let epoch = table.ptys[index].epoch.checked_add(1).ok_or(Error::Full)?;
    table.ptys[index] = Pty {
        epoch,
        occupied: true,
        master_refs: 1,
        slave_refs: 0,
        input: Buffer::empty(),
        output: Buffer::empty(),
        line: [0; LINE_SIZE],
        line_len: 0,
        eof: false,
    };
    Ok(PtyId { index, epoch })
}

pub fn open_slave(index: usize) -> Result<PtyId, Error> {
    let mut table = TABLE.lock();
    let pty = table.ptys.get_mut(index).ok_or(Error::Invalid)?;
    if !pty.occupied || pty.slave_refs == u16::MAX {
        return Err(Error::Invalid);
    }
    pty.slave_refs += 1;
    Ok(PtyId {
        index,
        epoch: pty.epoch,
    })
}

pub fn clone_master(id: PtyId) -> Result<PtyId, Error> {
    let mut table = TABLE.lock();
    let pty = table.get_mut(id)?;
    if pty.master_refs == u16::MAX {
        return Err(Error::Full);
    }
    pty.master_refs += 1;
    Ok(id)
}

pub fn clone_slave(id: PtyId) -> Result<PtyId, Error> {
    let mut table = TABLE.lock();
    let pty = table.get_mut(id)?;
    if pty.slave_refs == u16::MAX {
        return Err(Error::Full);
    }
    pty.slave_refs += 1;
    Ok(id)
}

pub fn close_master(id: PtyId) {
    close(id, true);
}

pub fn close_slave(id: PtyId) {
    close(id, false);
}

fn close(id: PtyId, master: bool) {
    let mut table = TABLE.lock();
    let Ok(pty) = table.get_mut(id) else { return };
    let refs = if master {
        &mut pty.master_refs
    } else {
        &mut pty.slave_refs
    };
    *refs = refs.saturating_sub(1);
    if pty.master_refs == 0 && pty.slave_refs == 0 {
        pty.occupied = false;
        pty.input = Buffer::empty();
        pty.output = Buffer::empty();
        pty.line_len = 0;
        pty.eof = false;
    }
}

pub fn read_master(id: PtyId, buffer: &mut [u8]) -> Result<usize, Error> {
    let mut table = TABLE.lock();
    let pty = table.get_mut(id)?;
    let mut count = 0;
    while count < buffer.len() {
        let Some(byte) = pty.output.pop() else { break };
        buffer[count] = byte;
        count += 1;
    }
    if count == 0 && pty.slave_refs == 0 {
        Err(Error::Closed)
    } else {
        Ok(count)
    }
}

pub fn read_slave(id: PtyId, buffer: &mut [u8]) -> Result<usize, Error> {
    let mut table = TABLE.lock();
    let pty = table.get_mut(id)?;
    let mut count = 0;
    while count < buffer.len() {
        let Some(byte) = pty.input.pop() else { break };
        buffer[count] = byte;
        count += 1;
    }
    if count == 0 && pty.eof {
        pty.eof = false;
    }
    Ok(count)
}

pub fn write_master(id: PtyId, buffer: &[u8]) -> Result<usize, Error> {
    let mut table = TABLE.lock();
    let pty = table.get_mut(id)?;
    let termios = crate::terminal::termios();
    let canonical = termios.local_flags & 0x2 != 0;
    let signals = termios.local_flags & 0x1 != 0;
    let echo = termios.local_flags & 0x8 != 0;
    let mut count = 0;
    for &byte in buffer {
        if signals && byte == termios.control_chars[0] {
            if let Some(group) = crate::terminal::foreground_pid() {
                let _ = crate::task::kill_process(0u64.wrapping_sub(group), 2);
            }
            continue;
        }
        if canonical && byte == termios.control_chars[2] {
            if pty.line_len != 0 {
                pty.line_len -= 1;
            }
            if echo {
                let _ = pty.output.push(8);
                let _ = pty.output.push(b' ');
                let _ = pty.output.push(8);
            }
            count += 1;
            continue;
        }
        if canonical && byte == termios.control_chars[4] {
            if pty.line_len == 0 {
                pty.eof = true;
            } else {
                for line_byte in pty.line[..pty.line_len].iter().copied() {
                    pty.input.push(line_byte).map_err(|_| Error::Full)?;
                }
                pty.line_len = 0;
            }
            count += 1;
            continue;
        }
        if canonical && byte != b'\n' {
            if pty.line_len == LINE_SIZE {
                return if count == 0 { Err(Error::Full) } else { Ok(count) };
            }
            pty.line[pty.line_len] = byte;
            pty.line_len += 1;
            if echo {
                let _ = pty.output.push(byte);
            }
        } else if !canonical {
            pty.input.push(byte).map_err(|_| Error::Full)?;
            if echo {
                let _ = pty.output.push(byte);
            }
        } else if echo {
            let _ = pty.output.push(byte);
        }
        if canonical && byte == b'\n' {
            for line_byte in pty.line[..pty.line_len].iter().copied() {
                pty.input.push(line_byte).map_err(|_| Error::Full)?;
            }
            pty.input.push(b'\n').map_err(|_| Error::Full)?;
            pty.line_len = 0;
        }
        count += 1;
    }
    Ok(count)
}

pub fn write_slave(id: PtyId, buffer: &[u8]) -> Result<usize, Error> {
    let mut table = TABLE.lock();
    let pty = table.get_mut(id)?;
    for &byte in buffer {
        pty.output.push(byte).map_err(|_| Error::Full)?;
    }
    Ok(buffer.len())
}

#[cfg(feature = "s03-process-lifecycle-test")]
pub fn self_test() -> bool {
    let Ok(master) = create() else { return false };
    let Ok(slave) = open_slave(master.index) else {
        close_master(master);
        return false;
    };
    let mut read = [0u8; 8];
    let first = write_master(master, b"abc") == Ok(3);
    let empty = read_slave(slave, &mut read) == Ok(0);
    let newline = write_master(master, b"\n") == Ok(1);
    let line = read_slave(slave, &mut read) == Ok(4) && &read[..4] == b"abc\n";
    let echo_read = read_master(master, &mut read) == Ok(4) && &read[..4] == b"abc\n";
    let output = write_slave(slave, b"ok") == Ok(2);
    let echoed = read_master(master, &mut read) == Ok(2) && &read[..2] == b"ok";
    crate::serial::write_line(format_args!(
        "[S03.PTY] first={} empty={} newline={} line={} echo_read={} output={} echoed={}",
        first, empty, newline, line, echo_read, output, echoed
    ));
    let ok = first && empty && newline && line && echo_read && output && echoed;
    close_slave(slave);
    close_master(master);
    ok
}
