//! WovenFS v1 — the native on-disk filesystem for WovenHat.
//!
//! The layout and the crash-recovery argument are specified in
//! `docs/wovenfs-on-disk-format.md`. The governing design constraint is that
//! no operation depends on more than one sector being written atomically:
//! every multi-sector change is staged in a write-ahead journal whose
//! completion is decided by a single commit sector, and the superblock is
//! never overwritten in place but alternated between two checkpoint slots.
//!
//! The implementation is deliberately bounded: fixed inode and directory
//! capacity, eight direct blocks per inode, and no heap allocation.

use crate::block::{BlockDevice, Error as BlockError, SECTOR_SIZE};

pub const SUPER_MAGIC: &[u8; 8] = b"WOVENFS1";
pub const JOURNAL_DESC_MAGIC: &[u8; 8] = b"WFSJDESC";
pub const JOURNAL_COMMIT_MAGIC: &[u8; 8] = b"WFSJCOMM";
pub const FORMAT_VERSION: u16 = 1;

/// Blocks one transaction may stage. The descriptor holds one 8-byte target
/// LBA per block and must fit in a single sector alongside its header.
pub const MAX_TXN_BLOCKS: usize = 32;
/// Descriptor sector + staged data sectors + commit sector.
pub const JOURNAL_BLOCKS: u32 = MAX_TXN_BLOCKS as u32 + 2;

pub const INODE_SIZE: usize = 128;
pub const INODES_PER_BLOCK: usize = SECTOR_SIZE / INODE_SIZE;
pub const DIRECT_BLOCKS: usize = 8;
pub const MAX_FILE_SIZE: usize = DIRECT_BLOCKS * SECTOR_SIZE;

pub const DIR_ENTRY_SIZE: usize = 64;
pub const DIR_ENTRIES_PER_BLOCK: usize = SECTOR_SIZE / DIR_ENTRY_SIZE;
pub const MAX_NAME: usize = 48;
pub const MAX_DIR_ENTRIES: usize = DIRECT_BLOCKS * DIR_ENTRIES_PER_BLOCK;

pub const KIND_FILE: u16 = 1;
pub const KIND_DIR: u16 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Device,
    Corrupt,
    NoSpace,
    NotFound,
    Exists,
    InvalidName,
    TooLarge,
    NotADirectory,
    NotEmpty,
}

impl From<BlockError> for Error {
    fn from(_: BlockError) -> Self {
        Error::Device
    }
}

fn fnv1a_seeded(seed: u64, bytes: &[u8]) -> u64 {
    bytes.iter().fold(seed, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn fnv1a(bytes: &[u8]) -> u64 {
    fnv1a_seeded(0xcbf2_9ce4_8422_2325, bytes)
}

fn read_u16(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn read_u32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn read_u64(bytes: &[u8], at: usize) -> u64 {
    let mut raw = [0u8; 8];
    raw.copy_from_slice(&bytes[at..at + 8]);
    u64::from_le_bytes(raw)
}

// --------------------------------------------------------------- superblock

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SuperBlock {
    pub sequence: u64,
    pub total_blocks: u64,
    pub journal_start: u64,
    pub journal_blocks: u32,
    pub bitmap_start: u64,
    pub bitmap_blocks: u32,
    pub inode_start: u64,
    pub inode_count: u32,
    pub data_start: u64,
    pub root_inode: u64,
}

const SUPER_CHECKSUM_AT: usize = 80;

impl SuperBlock {
    fn encode(&self, sector: &mut [u8; SECTOR_SIZE]) {
        sector.fill(0);
        sector[..8].copy_from_slice(SUPER_MAGIC);
        sector[8..10].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
        sector[12..20].copy_from_slice(&self.sequence.to_le_bytes());
        sector[20..28].copy_from_slice(&self.total_blocks.to_le_bytes());
        sector[28..36].copy_from_slice(&self.journal_start.to_le_bytes());
        sector[36..40].copy_from_slice(&self.journal_blocks.to_le_bytes());
        sector[40..48].copy_from_slice(&self.bitmap_start.to_le_bytes());
        sector[48..52].copy_from_slice(&self.bitmap_blocks.to_le_bytes());
        sector[52..60].copy_from_slice(&self.inode_start.to_le_bytes());
        sector[60..64].copy_from_slice(&self.inode_count.to_le_bytes());
        sector[64..72].copy_from_slice(&self.data_start.to_le_bytes());
        sector[72..80].copy_from_slice(&self.root_inode.to_le_bytes());
        let checksum = fnv1a(&sector[..SUPER_CHECKSUM_AT]);
        sector[SUPER_CHECKSUM_AT..SUPER_CHECKSUM_AT + 8]
            .copy_from_slice(&checksum.to_le_bytes());
    }

    fn decode(sector: &[u8; SECTOR_SIZE]) -> Option<Self> {
        if &sector[..8] != SUPER_MAGIC || read_u16(sector, 8) != FORMAT_VERSION {
            return None;
        }
        if read_u64(sector, SUPER_CHECKSUM_AT) != fnv1a(&sector[..SUPER_CHECKSUM_AT]) {
            return None;
        }
        Some(Self {
            sequence: read_u64(sector, 12),
            total_blocks: read_u64(sector, 20),
            journal_start: read_u64(sector, 28),
            journal_blocks: read_u32(sector, 36),
            bitmap_start: read_u64(sector, 40),
            bitmap_blocks: read_u32(sector, 48),
            inode_start: read_u64(sector, 52),
            inode_count: read_u32(sector, 60),
            data_start: read_u64(sector, 64),
            root_inode: read_u64(sector, 72),
        })
    }
}

// -------------------------------------------------------------------- inode

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Inode {
    pub mode: u32,
    pub links: u32,
    pub size: u64,
    pub created: u64,
    pub modified: u64,
    pub data_checksum: u64,
    pub direct: [u64; DIRECT_BLOCKS],
    pub kind: u16,
}

const INODE_CHECKSUM_AT: usize = 120;

impl Inode {
    pub const fn empty() -> Self {
        Self {
            mode: 0,
            links: 0,
            size: 0,
            created: 0,
            modified: 0,
            data_checksum: 0,
            direct: [0; DIRECT_BLOCKS],
            kind: 0,
        }
    }

    pub const fn is_used(&self) -> bool {
        self.mode != 0
    }

    fn encode(&self, raw: &mut [u8]) {
        raw[..INODE_SIZE].fill(0);
        raw[0..4].copy_from_slice(&self.mode.to_le_bytes());
        raw[4..8].copy_from_slice(&self.links.to_le_bytes());
        raw[8..16].copy_from_slice(&self.size.to_le_bytes());
        raw[16..24].copy_from_slice(&self.created.to_le_bytes());
        raw[24..32].copy_from_slice(&self.modified.to_le_bytes());
        raw[32..40].copy_from_slice(&self.data_checksum.to_le_bytes());
        for (index, block) in self.direct.iter().enumerate() {
            let at = 40 + index * 8;
            raw[at..at + 8].copy_from_slice(&block.to_le_bytes());
        }
        raw[104..106].copy_from_slice(&self.kind.to_le_bytes());
        let checksum = fnv1a(&raw[..INODE_CHECKSUM_AT]);
        raw[INODE_CHECKSUM_AT..INODE_CHECKSUM_AT + 8].copy_from_slice(&checksum.to_le_bytes());
    }

    fn decode(raw: &[u8]) -> Option<Self> {
        // A freshly zeroed inode slot is a free inode, not corruption. Format
        // zeroes the inode table rather than writing a checksummed empty inode
        // into every slot, so all-zero must decode as free.
        if raw[..INODE_SIZE].iter().all(|byte| *byte == 0) {
            return Some(Self::empty());
        }
        if read_u64(raw, INODE_CHECKSUM_AT) != fnv1a(&raw[..INODE_CHECKSUM_AT]) {
            return None;
        }
        let mut direct = [0u64; DIRECT_BLOCKS];
        for (index, slot) in direct.iter_mut().enumerate() {
            *slot = read_u64(raw, 40 + index * 8);
        }
        Some(Self {
            mode: read_u32(raw, 0),
            links: read_u32(raw, 4),
            size: read_u64(raw, 8),
            created: read_u64(raw, 16),
            modified: read_u64(raw, 24),
            data_checksum: read_u64(raw, 32),
            direct,
            kind: read_u16(raw, 104),
        })
    }
}

// ---------------------------------------------------------- directory entry

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DirEntry {
    pub inode: u64,
    pub kind: u16,
    pub name_len: u16,
    pub name: [u8; MAX_NAME],
}

impl DirEntry {
    pub const fn empty() -> Self {
        Self {
            inode: 0,
            kind: 0,
            name_len: 0,
            name: [0; MAX_NAME],
        }
    }

    pub fn matches(&self, name: &str) -> bool {
        self.inode != 0
            && usize::from(self.name_len) == name.len()
            && &self.name[..name.len()] == name.as_bytes()
    }

    pub fn name_str(&self) -> &str {
        let len = usize::from(self.name_len).min(MAX_NAME);
        core::str::from_utf8(&self.name[..len]).unwrap_or("")
    }

    fn encode(&self, raw: &mut [u8]) {
        raw[..DIR_ENTRY_SIZE].fill(0);
        raw[0..8].copy_from_slice(&self.inode.to_le_bytes());
        raw[8..10].copy_from_slice(&self.name_len.to_le_bytes());
        raw[10..12].copy_from_slice(&self.kind.to_le_bytes());
        raw[16..16 + MAX_NAME].copy_from_slice(&self.name);
    }

    fn decode(raw: &[u8]) -> Self {
        let mut name = [0u8; MAX_NAME];
        name.copy_from_slice(&raw[16..16 + MAX_NAME]);
        Self {
            inode: read_u64(raw, 0),
            name_len: read_u16(raw, 8),
            kind: read_u16(raw, 10),
            name,
        }
    }
}

// ------------------------------------------------------------------ journal

#[derive(Clone, Copy)]
struct StagedBlock {
    target: u64,
    data: [u8; SECTOR_SIZE],
}

/// A bounded write-ahead transaction.
///
/// Blocks accumulate in memory, are staged into the journal, become durable
/// when one commit sector lands, and only then are applied to their final
/// locations. Nothing reaches its target before the commit sector.
pub struct Transaction {
    blocks: [StagedBlock; MAX_TXN_BLOCKS],
    len: usize,
}

impl Default for Transaction {
    fn default() -> Self {
        Self::new()
    }
}

impl Transaction {
    pub const fn new() -> Self {
        Self {
            blocks: [StagedBlock {
                target: 0,
                data: [0; SECTOR_SIZE],
            }; MAX_TXN_BLOCKS],
            len: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Stage `data` for `target`, replacing any earlier staging of the same
    /// target so one transaction cannot hold two conflicting writes.
    pub fn stage(&mut self, target: u64, data: &[u8; SECTOR_SIZE]) -> Result<(), Error> {
        if let Some(existing) = self.blocks[..self.len]
            .iter_mut()
            .find(|block| block.target == target)
        {
            existing.data = *data;
            return Ok(());
        }
        if self.len == MAX_TXN_BLOCKS {
            return Err(Error::NoSpace);
        }
        self.blocks[self.len] = StagedBlock {
            target,
            data: *data,
        };
        self.len += 1;
        Ok(())
    }

    /// Most recent staged bytes for `target`, letting a caller observe its own
    /// uncommitted changes within the same transaction.
    fn staged(&self, target: u64) -> Option<&[u8; SECTOR_SIZE]> {
        self.blocks[..self.len]
            .iter()
            .find(|block| block.target == target)
            .map(|block| &block.data)
    }
}

fn staged_checksum(blocks: &[StagedBlock]) -> u64 {
    blocks.iter().fold(0xcbf2_9ce4_8422_2325, |hash, block| {
        fnv1a_seeded(hash, &block.data)
    })
}

// ------------------------------------------------------------------- volume

/// A mounted WovenFS volume.
///
/// Only the superblock is cached. Everything else is read through the device on
/// demand, so a crash can never leave stale in-memory state authoritative.
pub struct Volume<D: BlockDevice> {
    device: D,
    super_block: SuperBlock,
    /// Checkpoint slot holding the authoritative superblock (0 or 1).
    slot: u64,
}

/// Derive geometry from a device size.
fn plan(total_blocks: u64, inode_count: u32) -> Result<SuperBlock, Error> {
    let journal_start = 2;
    let journal_blocks = JOURNAL_BLOCKS;
    let bitmap_start = journal_start + u64::from(journal_blocks);
    let bitmap_blocks = total_blocks.div_ceil((SECTOR_SIZE * 8) as u64);
    let bitmap_blocks = u32::try_from(bitmap_blocks).map_err(|_| Error::TooLarge)?;
    let inode_start = bitmap_start + u64::from(bitmap_blocks);
    let inode_blocks = u64::from(inode_count).div_ceil(INODES_PER_BLOCK as u64);
    let data_start = inode_start + inode_blocks;
    if data_start >= total_blocks {
        return Err(Error::NoSpace);
    }
    Ok(SuperBlock {
        sequence: 1,
        total_blocks,
        journal_start,
        journal_blocks,
        bitmap_start,
        bitmap_blocks,
        inode_start,
        inode_count,
        data_start,
        root_inode: 1,
    })
}

fn bitmap_location(sb: &SuperBlock, block: u64) -> (u64, usize, u8) {
    let lba = sb.bitmap_start + block / (SECTOR_SIZE as u64 * 8);
    let within = (block % (SECTOR_SIZE as u64 * 8)) as usize;
    (lba, within / 8, 1u8 << (within % 8))
}

fn inode_location(sb: &SuperBlock, inode: u64) -> (u64, usize) {
    let index = inode - 1;
    let lba = sb.inode_start + index / INODES_PER_BLOCK as u64;
    let offset = (index % INODES_PER_BLOCK as u64) as usize * INODE_SIZE;
    (lba, offset)
}

/// Direct, unjournalled metadata write. Only `format` may use this, before any
/// superblock exists to make the volume mountable.
fn write_inode_raw<D: BlockDevice>(
    device: &mut D,
    sb: &SuperBlock,
    inode: u64,
    value: &Inode,
) -> Result<(), Error> {
    let (lba, offset) = inode_location(sb, inode);
    let mut sector = [0u8; SECTOR_SIZE];
    device.read_sector(lba, &mut sector)?;
    value.encode(&mut sector[offset..offset + INODE_SIZE]);
    device.write_sector(lba, &sector)?;
    Ok(())
}

fn set_bitmap_raw<D: BlockDevice>(
    device: &mut D,
    sb: &SuperBlock,
    block: u64,
    allocated: bool,
) -> Result<(), Error> {
    let (lba, byte, mask) = bitmap_location(sb, block);
    let mut sector = [0u8; SECTOR_SIZE];
    device.read_sector(lba, &mut sector)?;
    if allocated {
        sector[byte] |= mask;
    } else {
        sector[byte] &= !mask;
    }
    device.write_sector(lba, &sector)?;
    Ok(())
}

impl<D: BlockDevice> Volume<D> {
    /// Write a fresh filesystem over the whole device.
    ///
    /// Both superblock slots are cleared first and the real superblock is
    /// written last, so a crash part-way through leaves no valid superblock and
    /// the volume reads as unformatted rather than as a filesystem with garbage
    /// metadata.
    pub fn format(mut device: D, inode_count: u32) -> Result<Self, Error> {
        let total = device.sector_count();
        if total < 16 || inode_count == 0 {
            return Err(Error::NoSpace);
        }
        let sb = plan(total, inode_count)?;
        let zero = [0u8; SECTOR_SIZE];

        device.write_sector(0, &zero)?;
        device.write_sector(1, &zero)?;
        for lba in sb.journal_start..sb.data_start {
            device.write_sector(lba, &zero)?;
        }

        let root_block = sb.data_start;
        for block in 0..=root_block {
            set_bitmap_raw(&mut device, &sb, block, true)?;
        }
        device.write_sector(root_block, &zero)?;

        let mut root = Inode::empty();
        root.mode = 0o040_755;
        root.links = 2;
        root.kind = KIND_DIR;
        root.size = SECTOR_SIZE as u64;
        root.direct[0] = root_block;
        write_inode_raw(&mut device, &sb, sb.root_inode, &root)?;

        let mut sector = [0u8; SECTOR_SIZE];
        sb.encode(&mut sector);
        device.write_sector(0, &sector)?;
        device.flush()?;
        Ok(Self {
            device,
            super_block: sb,
            slot: 0,
        })
    }

    /// Mount an existing volume, replaying any committed transaction.
    pub fn mount(mut device: D) -> Result<Self, Error> {
        let mut best: Option<(u64, SuperBlock)> = None;
        for slot in 0..2u64 {
            let mut sector = [0u8; SECTOR_SIZE];
            if device.read_sector(slot, &mut sector).is_err() {
                continue;
            }
            // Flat rather than nested so this reads the same under the
            // edition-2021 kernel crate and the edition-2024 host test target,
            // which compiles this file directly and has let-chains available.
            let Some(candidate) = SuperBlock::decode(&sector) else {
                continue;
            };
            if best.is_none_or(|(_, current)| candidate.sequence > current.sequence) {
                best = Some((slot, candidate));
            }
        }
        let Some((slot, super_block)) = best else {
            return Err(Error::Corrupt);
        };
        let mut volume = Self {
            device,
            super_block,
            slot,
        };
        volume.replay()?;
        Ok(volume)
    }

    pub const fn super_block(&self) -> &SuperBlock {
        &self.super_block
    }

    pub fn into_device(self) -> D {
        self.device
    }

    /// Replay a committed but possibly unapplied transaction.
    ///
    /// Idempotent: re-applying staged bytes to their targets yields the same
    /// result, so a crash during replay is itself recoverable.
    fn replay(&mut self) -> Result<(), Error> {
        let sb = self.super_block;
        let mut descriptor = [0u8; SECTOR_SIZE];
        self.device.read_sector(sb.journal_start, &mut descriptor)?;
        if &descriptor[..8] != JOURNAL_DESC_MAGIC {
            return Ok(());
        }
        let txn = read_u64(&descriptor, 8);
        let count = read_u32(&descriptor, 16) as usize;
        if txn == 0 || count == 0 || count > MAX_TXN_BLOCKS {
            return Ok(());
        }

        // The commit sector is the sole authority on whether this happened.
        let commit_lba = sb.journal_start + 1 + count as u64;
        let mut commit = [0u8; SECTOR_SIZE];
        self.device.read_sector(commit_lba, &mut commit)?;
        if &commit[..8] != JOURNAL_COMMIT_MAGIC || read_u64(&commit, 8) != txn {
            return Ok(());
        }

        let mut blocks = [StagedBlock {
            target: 0,
            data: [0; SECTOR_SIZE],
        }; MAX_TXN_BLOCKS];
        for (index, slot) in blocks.iter_mut().take(count).enumerate() {
            let target = read_u64(&descriptor, 24 + index * 8);
            if target >= sb.total_blocks {
                return Ok(());
            }
            let mut data = [0u8; SECTOR_SIZE];
            self.device
                .read_sector(sb.journal_start + 1 + index as u64, &mut data)?;
            *slot = StagedBlock { target, data };
        }
        if read_u64(&commit, 16) != staged_checksum(&blocks[..count]) {
            // The staged payload does not match what was committed, so the
            // journal itself is torn. Discard rather than apply unknown bytes.
            return Ok(());
        }

        for block in &blocks[..count] {
            self.device.write_sector(block.target, &block.data)?;
        }
        self.device.flush()?;
        self.retire_journal()
    }

    /// Invalidate the descriptor so a replayed transaction is not applied again.
    fn retire_journal(&mut self) -> Result<(), Error> {
        let zero = [0u8; SECTOR_SIZE];
        self.device
            .write_sector(self.super_block.journal_start, &zero)?;
        self.device.flush()?;
        Ok(())
    }

    /// Stage, commit, apply and checkpoint one transaction.
    ///
    /// The ordering here is the whole crash-safety argument: nothing reaches a
    /// final location until the single commit sector has landed.
    fn commit(&mut self, txn: &Transaction) -> Result<(), Error> {
        if txn.is_empty() {
            return Ok(());
        }
        let sb = self.super_block;
        let count = txn.len;
        let transaction_id = sb.sequence.wrapping_add(1).max(1);

        let mut descriptor = [0u8; SECTOR_SIZE];
        descriptor[..8].copy_from_slice(JOURNAL_DESC_MAGIC);
        descriptor[8..16].copy_from_slice(&transaction_id.to_le_bytes());
        descriptor[16..20].copy_from_slice(&(count as u32).to_le_bytes());
        for (index, block) in txn.blocks[..count].iter().enumerate() {
            let at = 24 + index * 8;
            descriptor[at..at + 8].copy_from_slice(&block.target.to_le_bytes());
        }
        self.device.write_sector(sb.journal_start, &descriptor)?;

        for (index, block) in txn.blocks[..count].iter().enumerate() {
            self.device
                .write_sector(sb.journal_start + 1 + index as u64, &block.data)?;
        }
        self.device.flush()?;

        let mut commit = [0u8; SECTOR_SIZE];
        commit[..8].copy_from_slice(JOURNAL_COMMIT_MAGIC);
        commit[8..16].copy_from_slice(&transaction_id.to_le_bytes());
        commit[16..24].copy_from_slice(&staged_checksum(&txn.blocks[..count]).to_le_bytes());
        self.device
            .write_sector(sb.journal_start + 1 + count as u64, &commit)?;
        self.device.flush()?;

        for block in txn.blocks[..count].iter() {
            self.device.write_sector(block.target, &block.data)?;
        }
        self.device.flush()?;

        let mut next = sb;
        next.sequence = transaction_id;
        let slot = 1 - self.slot;
        let mut sector = [0u8; SECTOR_SIZE];
        next.encode(&mut sector);
        self.device.write_sector(slot, &sector)?;
        self.device.flush()?;
        self.super_block = next;
        self.slot = slot;
        self.retire_journal()
    }

    // ------------------------------------------------------------- reads

    fn read_block(&mut self, lba: u64) -> Result<[u8; SECTOR_SIZE], Error> {
        let mut sector = [0u8; SECTOR_SIZE];
        self.device.read_sector(lba, &mut sector)?;
        Ok(sector)
    }

    /// Read a block, preferring bytes staged in `txn` so a caller observes its
    /// own uncommitted changes.
    fn read_block_staged(
        &mut self,
        txn: &Transaction,
        lba: u64,
    ) -> Result<[u8; SECTOR_SIZE], Error> {
        if let Some(staged) = txn.staged(lba) {
            return Ok(*staged);
        }
        self.read_block(lba)
    }

    pub fn inode(&mut self, number: u64) -> Result<Inode, Error> {
        if number == 0 || number > u64::from(self.super_block.inode_count) {
            return Err(Error::NotFound);
        }
        let (lba, offset) = inode_location(&self.super_block, number);
        let sector = self.read_block(lba)?;
        Inode::decode(&sector[offset..offset + INODE_SIZE]).ok_or(Error::Corrupt)
    }

    fn inode_staged(&mut self, txn: &Transaction, number: u64) -> Result<Inode, Error> {
        if number == 0 || number > u64::from(self.super_block.inode_count) {
            return Err(Error::NotFound);
        }
        let (lba, offset) = inode_location(&self.super_block, number);
        let sector = self.read_block_staged(txn, lba)?;
        Inode::decode(&sector[offset..offset + INODE_SIZE]).ok_or(Error::Corrupt)
    }

    fn stage_inode(
        &mut self,
        txn: &mut Transaction,
        number: u64,
        value: &Inode,
    ) -> Result<(), Error> {
        let (lba, offset) = inode_location(&self.super_block, number);
        let mut sector = self.read_block_staged(txn, lba)?;
        value.encode(&mut sector[offset..offset + INODE_SIZE]);
        txn.stage(lba, &sector)
    }

    fn bitmap_get(&mut self, txn: &Transaction, block: u64) -> Result<bool, Error> {
        let (lba, byte, mask) = bitmap_location(&self.super_block, block);
        let sector = self.read_block_staged(txn, lba)?;
        Ok(sector[byte] & mask != 0)
    }

    fn stage_bitmap(
        &mut self,
        txn: &mut Transaction,
        block: u64,
        allocated: bool,
    ) -> Result<(), Error> {
        let (lba, byte, mask) = bitmap_location(&self.super_block, block);
        let mut sector = self.read_block_staged(txn, lba)?;
        if allocated {
            sector[byte] |= mask;
        } else {
            sector[byte] &= !mask;
        }
        txn.stage(lba, &sector)
    }

    fn allocate_block(&mut self, txn: &mut Transaction) -> Result<u64, Error> {
        let sb = self.super_block;
        for block in sb.data_start..sb.total_blocks {
            if !self.bitmap_get(txn, block)? {
                self.stage_bitmap(txn, block, true)?;
                return Ok(block);
            }
        }
        Err(Error::NoSpace)
    }

    fn allocate_inode(&mut self, txn: &mut Transaction) -> Result<u64, Error> {
        for number in 2..=u64::from(self.super_block.inode_count) {
            if !self.inode_staged(txn, number)?.is_used() {
                return Ok(number);
            }
        }
        Err(Error::NoSpace)
    }

    // -------------------------------------------------------- directories

    fn dir_entries(&mut self, txn: &Transaction, dir: &Inode) -> Result<DirectoryView, Error> {
        let mut view = DirectoryView::empty();
        for (slot, block) in dir.direct.iter().enumerate() {
            if *block == 0 {
                continue;
            }
            let sector = self.read_block_staged(txn, *block)?;
            for index in 0..DIR_ENTRIES_PER_BLOCK {
                let at = index * DIR_ENTRY_SIZE;
                view.entries[slot * DIR_ENTRIES_PER_BLOCK + index] =
                    DirEntry::decode(&sector[at..at + DIR_ENTRY_SIZE]);
            }
        }
        Ok(view)
    }

    /// Look up `name` in a directory inode.
    pub fn lookup(&mut self, dir_inode: u64, name: &str) -> Result<DirEntry, Error> {
        let txn = Transaction::new();
        let dir = self.inode(dir_inode)?;
        if dir.kind != KIND_DIR {
            return Err(Error::NotADirectory);
        }
        let view = self.dir_entries(&txn, &dir)?;
        view.entries
            .iter()
            .find(|entry| entry.matches(name))
            .copied()
            .ok_or(Error::NotFound)
    }

    /// Entries of a directory, free slots included as `inode == 0`.
    pub fn read_dir(&mut self, dir_inode: u64) -> Result<DirectoryView, Error> {
        let txn = Transaction::new();
        let dir = self.inode(dir_inode)?;
        if dir.kind != KIND_DIR {
            return Err(Error::NotADirectory);
        }
        self.dir_entries(&txn, &dir)
    }

    fn stage_dir_entry(
        &mut self,
        txn: &mut Transaction,
        dir: &Inode,
        position: usize,
        entry: &DirEntry,
    ) -> Result<(), Error> {
        let slot = position / DIR_ENTRIES_PER_BLOCK;
        let index = position % DIR_ENTRIES_PER_BLOCK;
        let block = dir.direct[slot];
        if block == 0 {
            return Err(Error::Corrupt);
        }
        let mut sector = self.read_block_staged(txn, block)?;
        let at = index * DIR_ENTRY_SIZE;
        entry.encode(&mut sector[at..at + DIR_ENTRY_SIZE]);
        txn.stage(block, &sector)
    }

    // ------------------------------------------------------------ mutation

    /// Create an empty file in `dir_inode`, as one transaction.
    pub fn create(&mut self, dir_inode: u64, name: &str, mode: u32, now: u64) -> Result<u64, Error> {
        if name.is_empty() || name.len() > MAX_NAME || name.contains('/') {
            return Err(Error::InvalidName);
        }
        let mut txn = Transaction::new();
        let mut dir = self.inode(dir_inode)?;
        if dir.kind != KIND_DIR {
            return Err(Error::NotADirectory);
        }
        let view = self.dir_entries(&txn, &dir)?;
        if view.entries.iter().any(|entry| entry.matches(name)) {
            return Err(Error::Exists);
        }
        let Some(position) = view
            .entries
            .iter()
            .enumerate()
            .filter(|(position, _)| dir.direct[position / DIR_ENTRIES_PER_BLOCK] != 0)
            .find(|(_, entry)| entry.inode == 0)
            .map(|(position, _)| position)
        else {
            return Err(Error::NoSpace);
        };

        let number = self.allocate_inode(&mut txn)?;
        let mut node = Inode::empty();
        node.mode = mode.max(1);
        node.links = 1;
        node.kind = KIND_FILE;
        node.created = now;
        node.modified = now;
        self.stage_inode(&mut txn, number, &node)?;

        let mut entry = DirEntry::empty();
        entry.inode = number;
        entry.kind = KIND_FILE;
        entry.name_len = name.len() as u16;
        entry.name[..name.len()].copy_from_slice(name.as_bytes());
        self.stage_dir_entry(&mut txn, &dir, position, &entry)?;

        dir.modified = now;
        self.stage_inode(&mut txn, dir_inode, &dir)?;

        self.commit(&txn)?;
        Ok(number)
    }

    /// Replace a file's contents, as one transaction.
    pub fn write_file(&mut self, inode: u64, data: &[u8], now: u64) -> Result<(), Error> {
        if data.len() > MAX_FILE_SIZE {
            return Err(Error::TooLarge);
        }
        let mut txn = Transaction::new();
        let mut node = self.inode_staged(&txn, inode)?;
        if !node.is_used() {
            return Err(Error::NotFound);
        }
        if node.kind != KIND_FILE {
            return Err(Error::NotADirectory);
        }

        let needed = data.len().div_ceil(SECTOR_SIZE);
        for slot in 0..DIRECT_BLOCKS {
            if slot < needed {
                if node.direct[slot] == 0 {
                    node.direct[slot] = self.allocate_block(&mut txn)?;
                }
                let mut sector = [0u8; SECTOR_SIZE];
                let from = slot * SECTOR_SIZE;
                let to = (from + SECTOR_SIZE).min(data.len());
                sector[..to - from].copy_from_slice(&data[from..to]);
                let block = node.direct[slot];
                txn.stage(block, &sector)?;
            } else if node.direct[slot] != 0 {
                let block = node.direct[slot];
                self.stage_bitmap(&mut txn, block, false)?;
                node.direct[slot] = 0;
            }
        }

        node.size = data.len() as u64;
        node.modified = now;
        node.data_checksum = fnv1a(data);
        self.stage_inode(&mut txn, inode, &node)?;
        self.commit(&txn)
    }

    /// Read a file's contents into `out`, returning the byte count.
    ///
    /// Fails closed on a content checksum mismatch rather than returning bytes
    /// that do not match what was written.
    pub fn read_file(&mut self, inode: u64, out: &mut [u8]) -> Result<usize, Error> {
        let node = self.inode(inode)?;
        if !node.is_used() {
            return Err(Error::NotFound);
        }
        let size = node.size as usize;
        if out.len() < size {
            return Err(Error::TooLarge);
        }
        let mut copied = 0;
        for slot in 0..DIRECT_BLOCKS {
            if copied >= size {
                break;
            }
            let block = node.direct[slot];
            if block == 0 {
                return Err(Error::Corrupt);
            }
            let sector = self.read_block(block)?;
            let take = (size - copied).min(SECTOR_SIZE);
            out[copied..copied + take].copy_from_slice(&sector[..take]);
            copied += take;
        }
        if fnv1a(&out[..size]) != node.data_checksum {
            return Err(Error::Corrupt);
        }
        Ok(size)
    }

    /// Remove a file from a directory and free its blocks, as one transaction.
    pub fn unlink(&mut self, dir_inode: u64, name: &str, now: u64) -> Result<(), Error> {
        let mut txn = Transaction::new();
        let mut dir = self.inode(dir_inode)?;
        if dir.kind != KIND_DIR {
            return Err(Error::NotADirectory);
        }
        let view = self.dir_entries(&txn, &dir)?;
        let Some((position, entry)) = view
            .entries
            .iter()
            .enumerate()
            .find(|(_, entry)| entry.matches(name))
            .map(|(position, entry)| (position, *entry))
        else {
            return Err(Error::NotFound);
        };

        let mut node = self.inode_staged(&txn, entry.inode)?;
        for slot in 0..DIRECT_BLOCKS {
            if node.direct[slot] != 0 {
                let block = node.direct[slot];
                self.stage_bitmap(&mut txn, block, false)?;
                node.direct[slot] = 0;
            }
        }
        self.stage_inode(&mut txn, entry.inode, &Inode::empty())?;
        self.stage_dir_entry(&mut txn, &dir, position, &DirEntry::empty())?;
        dir.modified = now;
        self.stage_inode(&mut txn, dir_inode, &dir)?;
        self.commit(&txn)
    }

    /// Verify internal consistency: every block referenced by a live inode is
    /// marked allocated, every directory entry names a live inode, and no two
    /// inodes claim the same block.
    pub fn check(&mut self) -> Result<(), Error> {
        let sb = self.super_block;
        let txn = Transaction::new();
        let mut seen = [0u64; 64];
        let mut seen_len = 0usize;

        for number in 1..=u64::from(sb.inode_count) {
            let node = self.inode(number)?;
            if !node.is_used() {
                continue;
            }
            for slot in 0..DIRECT_BLOCKS {
                let block = node.direct[slot];
                if block == 0 {
                    continue;
                }
                if block < sb.data_start || block >= sb.total_blocks {
                    return Err(Error::Corrupt);
                }
                if !self.bitmap_get(&txn, block)? {
                    return Err(Error::Corrupt);
                }
                if seen[..seen_len].contains(&block) {
                    return Err(Error::Corrupt);
                }
                if seen_len < seen.len() {
                    seen[seen_len] = block;
                    seen_len += 1;
                }
            }
        }

        let root = self.inode(sb.root_inode)?;
        if root.kind != KIND_DIR {
            return Err(Error::Corrupt);
        }
        let view = self.dir_entries(&txn, &root)?;
        for entry in view.entries.iter().filter(|entry| entry.inode != 0) {
            if entry.inode > u64::from(sb.inode_count) {
                return Err(Error::Corrupt);
            }
            if !self.inode(entry.inode)?.is_used() {
                return Err(Error::Corrupt);
            }
        }
        Ok(())
    }
}

/// A directory's entries, free slots included as `inode == 0`.
#[derive(Clone, Copy)]
pub struct DirectoryView {
    pub entries: [DirEntry; MAX_DIR_ENTRIES],
}

impl DirectoryView {
    pub const fn empty() -> Self {
        Self {
            entries: [DirEntry::empty(); MAX_DIR_ENTRIES],
        }
    }

    pub fn live(&self) -> impl Iterator<Item = &DirEntry> {
        self.entries.iter().filter(|entry| entry.inode != 0)
    }

    pub fn count(&self) -> usize {
        self.live().count()
    }
}

// ---------------------------------------------------------------- self-test

/// Heap-backed device for the acceptance self-test.
///
/// A fixed-capacity `RamDisk` of the size this test needs would be a ~100 KiB
/// stack frame, so the image lives on the heap. `budget` lets the self-test
/// stop accepting writes at an exact point and so reproduce a power loss inside
/// a transaction.
#[cfg(feature = "stage15-1-test")]
pub struct TestDisk {
    sectors: alloc::vec::Vec<[u8; SECTOR_SIZE]>,
    budget: Option<usize>,
    accepted: usize,
}

#[cfg(feature = "stage15-1-test")]
impl TestDisk {
    pub fn new(sectors: usize) -> Self {
        Self {
            sectors: alloc::vec![[0; SECTOR_SIZE]; sectors],
            budget: None,
            accepted: 0,
        }
    }

    fn fork(&self, budget: Option<usize>) -> Self {
        Self {
            sectors: self.sectors.clone(),
            budget,
            accepted: 0,
        }
    }

    pub const fn accepted(&self) -> usize {
        self.accepted
    }
}

#[cfg(feature = "stage15-1-test")]
impl BlockDevice for TestDisk {
    fn sector_count(&self) -> u64 {
        self.sectors.len() as u64
    }

    fn read_sector(&mut self, lba: u64, sector: &mut [u8]) -> Result<(), BlockError> {
        if sector.len() != SECTOR_SIZE {
            return Err(BlockError::InvalidBuffer);
        }
        let index = usize::try_from(lba).map_err(|_| BlockError::OutOfBounds)?;
        let source = self.sectors.get(index).ok_or(BlockError::OutOfBounds)?;
        sector.copy_from_slice(source);
        Ok(())
    }

    fn write_sector(&mut self, lba: u64, sector: &[u8]) -> Result<(), BlockError> {
        if sector.len() != SECTOR_SIZE {
            return Err(BlockError::InvalidBuffer);
        }
        let index = usize::try_from(lba).map_err(|_| BlockError::OutOfBounds)?;
        if index >= self.sectors.len() {
            return Err(BlockError::OutOfBounds);
        }
        if let Some(remaining) = self.budget.as_mut() {
            if *remaining == 0 {
                return Err(BlockError::DeviceFault);
            }
            *remaining -= 1;
        }
        self.sectors[index].copy_from_slice(sector);
        self.accepted += 1;
        Ok(())
    }
}

/// Stage 15.1 acceptance: format, file lifecycle, and crash recovery proved by
/// enumerating every write at which a transaction can lose power.
#[cfg(feature = "stage15-1-test")]
pub fn stage15_1_self_test() -> bool {
    const SECTORS: usize = 192;
    const INODES: u32 = 32;

    // Format, then confirm the volume mounts clean and empty.
    let Ok(volume) = Volume::format(TestDisk::new(SECTORS), INODES) else {
        return false;
    };
    let base = volume.into_device();
    let Ok(mut mounted) = Volume::mount(base.fork(None)) else {
        return false;
    };
    if mounted.check().is_err() {
        return false;
    }
    if mounted.read_dir(1).map(|view| view.count()) != Ok(0) {
        return false;
    }

    // Full lifecycle: create, write, read back, unlink.
    let payload: [u8; 1300] = {
        let mut bytes = [0u8; 1300];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = (index % 251) as u8;
        }
        bytes
    };
    let Ok(inode) = mounted.create(1, "report.bin", 0o644, 10) else {
        return false;
    };
    if mounted.write_file(inode, &payload, 11).is_err() {
        return false;
    }
    let mut out = [0u8; MAX_FILE_SIZE];
    if mounted.read_file(inode, &mut out) != Ok(payload.len()) {
        return false;
    }
    if out[..payload.len()] != payload {
        return false;
    }
    if mounted.check().is_err() {
        return false;
    }

    // Contents must survive an unmount/remount cycle.
    let populated = mounted.into_device();
    let Ok(mut remounted) = Volume::mount(populated.fork(None)) else {
        return false;
    };
    let Ok(entry) = remounted.lookup(1, "report.bin") else {
        return false;
    };
    let mut again = [0u8; MAX_FILE_SIZE];
    if remounted.read_file(entry.inode, &mut again) != Ok(payload.len())
        || again[..payload.len()] != payload
    {
        return false;
    }
    if remounted.unlink(1, "report.bin", 12).is_err()
        || remounted.lookup(1, "report.bin") != Err(Error::NotFound)
        || remounted.check().is_err()
    {
        return false;
    }

    // Crash recovery. Count the writes one create performs, then replay the
    // operation once per write boundary, cutting power at that exact write, and
    // require the result to be consistent and all-or-nothing every time.
    let counted = {
        let Ok(mut probe) = Volume::mount(base.fork(None)) else {
            return false;
        };
        if probe.create(1, "atomic.bin", 0o644, 20).is_err() {
            return false;
        }
        probe.into_device().accepted()
    };
    if counted < 3 {
        return false;
    }

    for cut in 0..=counted {
        let Ok(mut crashing) = Volume::mount(base.fork(Some(cut))) else {
            return false;
        };
        let _ = crashing.create(1, "atomic.bin", 0o644, 20);
        let image = crashing.into_device();

        let Ok(mut recovered) = Volume::mount(image.fork(None)) else {
            return false;
        };
        if recovered.check().is_err() {
            return false;
        }
        match recovered.lookup(1, "atomic.bin") {
            // Present must mean completely present.
            Ok(found) => match recovered.inode(found.inode) {
                Ok(node) if node.is_used() && node.size == 0 => {}
                _ => return false,
            },
            Err(Error::NotFound) => {}
            Err(_) => return false,
        }

        // Replay must be idempotent: a second and third mount change nothing.
        let once = recovered.into_device();
        let Ok(twice) = Volume::mount(once.fork(None)) else {
            return false;
        };
        let twice = twice.into_device();
        let Ok(thrice) = Volume::mount(twice.fork(None)) else {
            return false;
        };
        if twice.sectors != thrice.into_device().sectors {
            return false;
        }
    }

    // A destroyed checkpoint slot must fall back to the other one.
    let Ok(mut volume) = Volume::mount(base.fork(None)) else {
        return false;
    };
    if volume.create(1, "keep.bin", 0o644, 30).is_err() {
        return false;
    }
    let mut damaged = volume.into_device();
    damaged.sectors[1] = [0xab; SECTOR_SIZE];
    let Ok(mut survivor) = Volume::mount(damaged.fork(None)) else {
        return false;
    };
    survivor.check().is_ok()
}
