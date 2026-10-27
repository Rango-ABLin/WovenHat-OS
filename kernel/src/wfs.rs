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
#[cfg(any(test, feature = "stage15-1-test", feature = "stage15-2-test"))]
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
    #[cfg(any(test, feature = "stage15-1-test", feature = "stage15-2-test"))]
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
        sector[SUPER_CHECKSUM_AT..SUPER_CHECKSUM_AT + 8].copy_from_slice(&checksum.to_le_bytes());
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

    /// Whether the regions this superblock describes fit the device in order
    /// and without overlap: slots, journal, bitmap, inode table, data.
    ///
    /// The checksum is unkeyed, so it proves only that the sector was written
    /// whole. Every later bounds check (inode pointers, bitmap and inode
    /// locations, journal replay targets) is relative to this geometry, so an
    /// unchecked geometry would let a crafted image aim them at metadata or
    /// past the device.
    fn geometry_fits(&self, device_sectors: u64) -> bool {
        let end = |start: u64, len: u64| start.checked_add(len);
        let inode_blocks = u64::from(self.inode_count).div_ceil(INODES_PER_BLOCK as u64);
        let bitmap_needed = self.total_blocks.div_ceil((SECTOR_SIZE * 8) as u64);
        // Replay reads the descriptor, up to MAX_TXN_BLOCKS staged sectors and
        // the commit sector, all inside the journal.
        let journal_needed = MAX_TXN_BLOCKS as u64 + 2;
        let (Some(journal_end), Some(bitmap_end), Some(inode_end)) = (
            end(self.journal_start, u64::from(self.journal_blocks)),
            end(self.bitmap_start, u64::from(self.bitmap_blocks)),
            end(self.inode_start, inode_blocks),
        ) else {
            return false;
        };
        self.journal_start >= 2
            && u64::from(self.journal_blocks) >= journal_needed
            && journal_end <= self.bitmap_start
            && u64::from(self.bitmap_blocks) >= bitmap_needed
            && bitmap_end <= self.inode_start
            && self.inode_count > 0
            && inode_end <= self.data_start
            && self.data_start < self.total_blocks
            && self.total_blocks <= device_sectors
            && (1..=u64::from(self.inode_count)).contains(&self.root_inode)
    }

    /// Whether a transaction may write `lba`: the bitmap, inode table and
    /// data regions. The superblock slots are written only by the checkpoint
    /// and the journal only by `commit` itself, so a staged target in either
    /// would let one transaction overwrite a checkpoint or the very journal
    /// sectors replay is still reading.
    fn journal_may_target(&self, lba: u64) -> bool {
        (self.bitmap_start..self.total_blocks).contains(&lba)
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

    /// `name_len` comes from an unchecksummed directory block, so a value past
    /// the name field must not match rather than index out of bounds.
    pub fn matches(&self, name: &str) -> bool {
        self.inode != 0
            && usize::from(self.name_len) == name.len()
            && self.name.get(..name.len()) == Some(name.as_bytes())
    }

    #[cfg(any(test, feature = "stage15-1-test", feature = "stage15-2-test"))]
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

    #[cfg(any(test, feature = "stage15-1-test", feature = "stage15-2-test"))]
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

/// Commit checksum over the descriptor's used bytes (header and target list),
/// then every staged data sector. Covering the targets means a descriptor
/// damaged after commit is discarded on replay rather than writing committed
/// bytes to the wrong block.
fn journal_checksum(descriptor: &[u8; SECTOR_SIZE], blocks: &[StagedBlock]) -> u64 {
    let header = fnv1a(&descriptor[..24 + blocks.len() * 8]);
    blocks
        .iter()
        .fold(header, |hash, block| fnv1a_seeded(hash, &block.data))
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
#[cfg(any(test, feature = "stage15-1-test", feature = "stage15-2-test"))]
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
#[cfg(any(test, feature = "stage15-1-test", feature = "stage15-2-test"))]
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

#[cfg(any(test, feature = "stage15-1-test", feature = "stage15-2-test"))]
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
    #[cfg(any(test, feature = "stage15-1-test", feature = "stage15-2-test"))]
    pub fn format(mut device: D, inode_count: u32) -> Result<Self, Error> {
        let total = device.sector_count();
        if total < 16 || inode_count == 0 {
            return Err(Error::NoSpace);
        }
        let sb = plan(total, inode_count)?;
        let zero = [0u8; SECTOR_SIZE];

        // Report the LBA a failure happened at. On a real device "Device" alone
        // is not actionable: a format touches thousands of sectors and the
        // serial log is the only evidence available.
        fn at(lba: u64, phase: &str, result: Result<(), BlockError>) -> Result<(), Error> {
            match result {
                Ok(()) => Ok(()),
                Err(_) => {
                    #[cfg(feature = "stage15-2-test")]
                    crate::serial::write_line(format_args!(
                        "[S15.2] format io error: phase {} lba {}",
                        phase, lba
                    ));
                    let _ = (lba, phase);
                    Err(Error::Device)
                }
            }
        }

        at(0, "super-a", device.write_sector(0, &zero))?;
        at(1, "super-b", device.write_sector(1, &zero))?;

        // Journal region only. The bitmap and inode regions are written with
        // their real contents below, so zeroing them separately would be a
        // second full pass over the same sectors.
        for lba in sb.journal_start..sb.bitmap_start {
            at(lba, "journal", device.write_sector(lba, &zero))?;
        }

        // Build each bitmap sector in memory and write it exactly once.
        //
        // The previous form set one bit per call through a read-modify-write of
        // the containing sector. Every metadata block maps into the first few
        // bitmap sectors, so on a large volume that re-read and re-wrote the
        // same sector thousands of times: 7770 operations on an 8 GB device
        // against 3840 sectors of actual bitmap.
        let root_block = sb.data_start;
        let bits_per_sector = (SECTOR_SIZE * 8) as u64;
        for index in 0..u64::from(sb.bitmap_blocks) {
            let mut sector = [0u8; SECTOR_SIZE];
            let first = index * bits_per_sector;
            if first <= root_block {
                let last = (first + bits_per_sector - 1).min(root_block);
                for block in first..=last {
                    let within = (block - first) as usize;
                    sector[within / 8] |= 1u8 << (within % 8);
                }
            }
            let lba = sb.bitmap_start + index;
            at(lba, "bitmap", device.write_sector(lba, &sector))?;
        }

        // Inode table, then the root directory's first block.
        for lba in sb.inode_start..sb.data_start {
            at(lba, "inodes", device.write_sector(lba, &zero))?;
        }
        at(
            root_block,
            "root-dir",
            device.write_sector(root_block, &zero),
        )?;

        let mut root = Inode::empty();
        root.mode = 0o040_755;
        root.links = 2;
        root.kind = KIND_DIR;
        root.size = SECTOR_SIZE as u64;
        root.direct[0] = root_block;
        let (inode_lba, inode_offset) = inode_location(&sb, sb.root_inode);
        let mut inode_sector = [0u8; SECTOR_SIZE];
        root.encode(&mut inode_sector[inode_offset..inode_offset + INODE_SIZE]);
        at(
            inode_lba,
            "root-inode",
            device.write_sector(inode_lba, &inode_sector),
        )?;

        // The superblock lands last. Before this write there is no filesystem,
        // so a crash at any earlier point leaves the volume unformatted rather
        // than half-built.
        let mut sector = [0u8; SECTOR_SIZE];
        sb.encode(&mut sector);
        at(0, "superblock", device.write_sector(0, &sector))?;
        at(0, "flush", device.flush())?;
        Ok(Self {
            device,
            super_block: sb,
            slot: 0,
        })
    }

    /// Mount an existing volume, replaying any committed transaction.
    pub fn mount(mut device: D) -> Result<Self, Error> {
        let device_sectors = device.sector_count();
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
            // A slot whose geometry does not fit is discarded exactly like a
            // slot whose checksum fails.
            if !candidate.geometry_fits(device_sectors) {
                continue;
            }
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

    #[cfg(any(test, feature = "stage15-1-test", feature = "stage15-2-test"))]
    pub const fn super_block(&self) -> &SuperBlock {
        &self.super_block
    }

    #[cfg(any(test, feature = "stage15-1-test", feature = "stage15-2-test"))]
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
            // `commit` never stages a target outside the transactional
            // regions, so one here is discarded rather than applied over a
            // checkpoint or the journal, independent of the checksum.
            if !sb.journal_may_target(target) {
                return Ok(());
            }
            let mut data = [0u8; SECTOR_SIZE];
            self.device
                .read_sector(sb.journal_start + 1 + index as u64, &mut data)?;
            *slot = StagedBlock { target, data };
        }
        if read_u64(&commit, 16) != journal_checksum(&descriptor, &blocks[..count]) {
            // The descriptor or staged payload does not match what was
            // committed, so the journal itself is torn. Discard rather than
            // apply unknown bytes, or known bytes to unknown targets.
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
        // Every staged target is derived from this geometry, so this fails
        // only on an internal bug. Refuse before the journal is touched:
        // replay would otherwise discard a transaction already reported as
        // committed.
        if !txn.blocks[..count]
            .iter()
            .all(|block| sb.journal_may_target(block.target))
        {
            return Err(Error::Corrupt);
        }
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
        commit[16..24]
            .copy_from_slice(&journal_checksum(&descriptor, &txn.blocks[..count]).to_le_bytes());
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
        self.checked_inode(&sector[offset..offset + INODE_SIZE])
    }

    fn inode_staged(&mut self, txn: &Transaction, number: u64) -> Result<Inode, Error> {
        if number == 0 || number > u64::from(self.super_block.inode_count) {
            return Err(Error::NotFound);
        }
        let (lba, offset) = inode_location(&self.super_block, number);
        let sector = self.read_block_staged(txn, lba)?;
        self.checked_inode(&sector[offset..offset + INODE_SIZE])
    }

    /// Decode an inode, refusing one whose block pointers leave the data
    /// region.
    ///
    /// The inode checksum detects bit rot, not a mis-written or crafted image.
    /// Writes reuse an inode's existing blocks in place and unlink clears their
    /// bitmap bits, so a pointer at the superblock, journal, bitmap or inode
    /// table would let file data overwrite metadata or free a metadata block.
    /// Free inodes are checked too, because a stale directory entry can still
    /// reach one through `unlink`. Two slots naming the same block are refused
    /// too: a write would stage that block twice with different bytes, and a
    /// shrink would free it while another slot still names it.
    fn checked_inode(&self, raw: &[u8]) -> Result<Inode, Error> {
        let node = Inode::decode(raw).ok_or(Error::Corrupt)?;
        let sb = &self.super_block;
        let in_data = |block: &u64| *block == 0 || (sb.data_start..sb.total_blocks).contains(block);
        if !node.direct.iter().all(in_data) {
            return Err(Error::Corrupt);
        }
        for (slot, block) in node.direct.iter().enumerate() {
            if *block != 0 && node.direct[slot + 1..].contains(block) {
                return Err(Error::Corrupt);
            }
        }
        if node.is_used()
            && (!matches!(node.kind, KIND_FILE | KIND_DIR) || node.size > MAX_FILE_SIZE as u64)
        {
            return Err(Error::Corrupt);
        }
        Ok(node)
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

    /// Stage `block` as free, refusing a block the bitmap already marks free.
    ///
    /// Only a block some inode still names is ever freed. A clear bit means the
    /// allocator may already have handed it to another inode, which would then
    /// lose a block it owns.
    fn stage_free_block(&mut self, txn: &mut Transaction, block: u64) -> Result<(), Error> {
        if !self.bitmap_get(txn, block)? {
            return Err(Error::Corrupt);
        }
        self.stage_bitmap(txn, block, false)
    }

    /// Refuse `blocks` if a live inode other than `owner` names any of them.
    ///
    /// The bitmap records only that a block is allocated, not to whom, so two
    /// inodes naming one allocated block (or a live inode naming a block the
    /// bitmap calls free, which the allocator may hand out) pass every
    /// per-inode check. Rewriting such a block in place would overwrite the
    /// other owner's data, and freeing it would leave the other owner pointing
    /// at a block the allocator can reuse. The scan reads the inode table
    /// through `txn`, so inodes staged earlier in the same transaction count.
    fn refuse_shared_blocks(
        &mut self,
        txn: &Transaction,
        owner: u64,
        blocks: &[u64],
    ) -> Result<(), Error> {
        for number in 1..=u64::from(self.super_block.inode_count) {
            if number == owner {
                continue;
            }
            let other = self.inode_staged(txn, number)?;
            if other.is_used()
                && other
                    .direct
                    .iter()
                    .any(|block| *block != 0 && blocks.contains(block))
            {
                return Err(Error::Corrupt);
            }
        }
        Ok(())
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
        let entry = view
            .entries
            .iter()
            .find(|entry| entry.matches(name))
            .copied()
            .ok_or(Error::NotFound)?;
        // A stale entry may name a free inode (e.g. after a crash before the
        // directory entry was cleared). `check()` flags this as corrupt; refuse
        // it here so callers never operate on a freed inode.
        if entry.inode > u64::from(self.super_block.inode_count)
            || !self.inode(entry.inode)?.is_used()
        {
            return Err(Error::Corrupt);
        }
        Ok(entry)
    }

    /// Entries of a directory, free slots included as `inode == 0`.
    ///
    /// Entries whose inode number is non-zero but names a free inode (stale
    /// entries left by a partial unlink or corruption) are zeroed before
    /// returning, so `live()` and `count()` agree with `lookup()`.
    #[cfg(any(test, feature = "stage15-1-test", feature = "stage15-2-test"))]
    pub fn read_dir(&mut self, dir_inode: u64) -> Result<DirectoryView, Error> {
        let txn = Transaction::new();
        let dir = self.inode(dir_inode)?;
        if dir.kind != KIND_DIR {
            return Err(Error::NotADirectory);
        }
        let mut view = self.dir_entries(&txn, &dir)?;
        // Mirror the guard in `lookup()`: a non-zero inode that is out of range
        // or whose mode is 0 is a stale entry.  Zero it so callers using
        // `live()` or `count()` see a consistent picture.
        for entry in view.entries.iter_mut() {
            if entry.inode != 0 {
                let valid = self
                    .inode(entry.inode)
                    .map(|n| n.is_used())
                    .unwrap_or(false);
                if !valid {
                    *entry = DirEntry::empty();
                }
            }
        }
        Ok(view)
    }

    /// Stage one entry of directory `dir_inode` (whose inode is `dir`).
    ///
    /// The entry's block is rewritten in place, so it gets the same ownership
    /// checks as a file block rewritten by `stage_write`. A file inode naming
    /// that block would otherwise have its data overwritten by a `create` or
    /// `unlink` that never touches the file, and a block the bitmap calls free
    /// may already belong to someone else.
    fn stage_dir_entry(
        &mut self,
        txn: &mut Transaction,
        dir_inode: u64,
        dir: &Inode,
        position: usize,
        entry: &DirEntry,
    ) -> Result<(), Error> {
        let slot = position / DIR_ENTRIES_PER_BLOCK;
        let index = position % DIR_ENTRIES_PER_BLOCK;
        let block = dir.direct[slot];
        if block == 0 || !self.bitmap_get(txn, block)? {
            return Err(Error::Corrupt);
        }
        self.refuse_shared_blocks(txn, dir_inode, &[block])?;
        let mut sector = self.read_block_staged(txn, block)?;
        let at = index * DIR_ENTRY_SIZE;
        entry.encode(&mut sector[at..at + DIR_ENTRY_SIZE]);
        txn.stage(block, &sector)
    }

    // ------------------------------------------------------------ mutation

    /// Create an empty file in `dir_inode`, as one transaction.
    pub fn create(
        &mut self,
        dir_inode: u64,
        name: &str,
        mode: u32,
        now: u64,
    ) -> Result<u64, Error> {
        let mut txn = Transaction::new();
        let number = self.stage_create(&mut txn, dir_inode, name, mode, now)?;
        self.commit(&txn)?;
        Ok(number)
    }

    /// Create a file in `dir_inode` holding `data`, as one transaction.
    ///
    /// `create` followed by `write_file` commits the empty file first, so a
    /// write that then fails (no space, device error) leaves an empty file
    /// behind although the caller reports failure. Here the file either
    /// appears with its contents or does not appear at all.
    pub fn create_file(
        &mut self,
        dir_inode: u64,
        name: &str,
        mode: u32,
        data: &[u8],
        now: u64,
    ) -> Result<u64, Error> {
        if data.len() > MAX_FILE_SIZE {
            return Err(Error::TooLarge);
        }
        let mut txn = Transaction::new();
        let number = self.stage_create(&mut txn, dir_inode, name, mode, now)?;
        self.stage_write(&mut txn, number, data, now)?;
        self.commit(&txn)?;
        Ok(number)
    }

    /// Stage a new empty file and its directory entry into `txn`.
    fn stage_create(
        &mut self,
        txn: &mut Transaction,
        dir_inode: u64,
        name: &str,
        mode: u32,
        now: u64,
    ) -> Result<u64, Error> {
        if name.is_empty() || name.len() > MAX_NAME || name.contains('/') {
            return Err(Error::InvalidName);
        }
        let mut dir = self.inode_staged(txn, dir_inode)?;
        if dir.kind != KIND_DIR {
            return Err(Error::NotADirectory);
        }
        let view = self.dir_entries(txn, &dir)?;
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

        let number = self.allocate_inode(txn)?;
        // Directory blocks carry no checksum, so a stale or crafted entry can
        // name a free inode. Handing that inode to the new file would give it
        // two names: the old one would read and overwrite the new file's
        // data, and unlink would refuse both as duplicates. Fail closed rather
        // than skip the inode, which would leave the dangling entry in place.
        // The root is the only directory, so `view` holds every entry.
        if view.entries.iter().any(|entry| entry.inode == number) {
            return Err(Error::Corrupt);
        }
        let mut node = Inode::empty();
        node.mode = mode.max(1);
        node.links = 1;
        node.kind = KIND_FILE;
        node.created = now;
        node.modified = now;
        self.stage_inode(txn, number, &node)?;

        let mut entry = DirEntry::empty();
        entry.inode = number;
        entry.kind = KIND_FILE;
        entry.name_len = name.len() as u16;
        entry.name[..name.len()].copy_from_slice(name.as_bytes());
        self.stage_dir_entry(txn, dir_inode, &dir, position, &entry)?;

        dir.modified = now;
        self.stage_inode(txn, dir_inode, &dir)?;
        Ok(number)
    }

    /// Replace a file's contents, as one transaction.
    pub fn write_file(&mut self, inode: u64, data: &[u8], now: u64) -> Result<(), Error> {
        let mut txn = Transaction::new();
        self.stage_write(&mut txn, inode, data, now)?;
        self.commit(&txn)
    }

    /// Stage a replacement of a file's contents into `txn`.
    fn stage_write(
        &mut self,
        txn: &mut Transaction,
        inode: u64,
        data: &[u8],
        now: u64,
    ) -> Result<(), Error> {
        if data.len() > MAX_FILE_SIZE {
            return Err(Error::TooLarge);
        }
        let mut node = self.inode_staged(txn, inode)?;
        if !node.is_used() {
            return Err(Error::NotFound);
        }
        if node.kind != KIND_FILE {
            return Err(Error::NotADirectory);
        }

        // Every block this write rewrites, frees or newly takes.
        let mut touched = [0u64; 2 * DIRECT_BLOCKS];
        touched[..DIRECT_BLOCKS].copy_from_slice(&node.direct);
        let needed = data.len().div_ceil(SECTOR_SIZE);
        for slot in 0..DIRECT_BLOCKS {
            if slot < needed {
                if node.direct[slot] == 0 {
                    node.direct[slot] = self.allocate_block(txn)?;
                } else if !self.bitmap_get(txn, node.direct[slot])? {
                    // Rewriting in place a block the bitmap calls free would
                    // overwrite whatever the allocator has since given it to.
                    return Err(Error::Corrupt);
                }
                let mut sector = [0u8; SECTOR_SIZE];
                let from = slot * SECTOR_SIZE;
                let to = (from + SECTOR_SIZE).min(data.len());
                sector[..to - from].copy_from_slice(&data[from..to]);
                let block = node.direct[slot];
                txn.stage(block, &sector)?;
            } else if node.direct[slot] != 0 {
                let block = node.direct[slot];
                self.stage_free_block(txn, block)?;
                node.direct[slot] = 0;
            }
        }

        touched[DIRECT_BLOCKS..].copy_from_slice(&node.direct);
        self.refuse_shared_blocks(txn, inode, &touched)?;

        node.size = data.len() as u64;
        node.modified = now;
        node.data_checksum = fnv1a(data);
        self.stage_inode(txn, inode, &node)
    }

    /// Read a file's contents into `out`, returning the byte count.
    ///
    /// Fails closed on a content checksum mismatch rather than returning bytes
    /// that do not match what was written. Refuses a directory inode the same
    /// way `write_file` does: directory blocks are raw entry tables, and a
    /// directory's `data_checksum` is never maintained, so a crafted value
    /// would otherwise export the entry table as file contents.
    pub fn read_file(&mut self, inode: u64, out: &mut [u8]) -> Result<usize, Error> {
        let node = self.inode(inode)?;
        if !node.is_used() {
            return Err(Error::NotFound);
        }
        if node.kind != KIND_FILE {
            return Err(Error::NotADirectory);
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
        // An empty file is created by `stage_create` with `data_checksum = 0`,
        // not the FNV-1a initial value; the checksum has no meaning until the
        // first write. `check()` documents and respects the same invariant.
        if size > 0 && fnv1a(&out[..size]) != node.data_checksum {
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

        // Directory blocks carry no checksum. Before freeing anything, require
        // that the entry names a live file inode and is the only entry that
        // does. An entry naming the directory would free the directory's own
        // blocks, and a second entry naming the same inode would be left
        // pointing at whatever file next reuses that inode number.
        if entry.inode == dir_inode
            || entry.inode > u64::from(self.super_block.inode_count)
            || view
                .entries
                .iter()
                .filter(|other| other.inode == entry.inode)
                .count()
                != 1
        {
            return Err(Error::Corrupt);
        }
        // WovenFS has no hard links: create writes `links = 1` and nothing
        // changes it. Any other count means some other name may still reach
        // the inode (or the image is crafted), and freeing the blocks would
        // destroy that file's data. Fail closed rather than guess; supporting
        // hard links later means decrementing here instead of freeing.
        let mut node = self.inode_staged(&txn, entry.inode)?;
        if !node.is_used() || node.kind != KIND_FILE || node.links != 1 {
            return Err(Error::Corrupt);
        }
        self.refuse_shared_blocks(&txn, entry.inode, &node.direct)?;
        for slot in 0..DIRECT_BLOCKS {
            if node.direct[slot] != 0 {
                let block = node.direct[slot];
                self.stage_free_block(&mut txn, block)?;
                node.direct[slot] = 0;
            }
        }
        self.stage_inode(&mut txn, entry.inode, &Inode::empty())?;
        self.stage_dir_entry(&mut txn, dir_inode, &dir, position, &DirEntry::empty())?;
        dir.modified = now;
        self.stage_inode(&mut txn, dir_inode, &dir)?;
        self.commit(&txn)
    }

    /// Verify internal consistency: every live file inode has one link, every
    /// block referenced by a live inode is marked allocated, every directory
    /// entry names a live inode, and no two inodes claim the same block.
    #[cfg(any(test, feature = "stage15-1-test", feature = "stage15-2-test"))]
    pub fn check(&mut self) -> Result<(), Error> {
        let sb = self.super_block;
        let txn = Transaction::new();

        for number in 1..=u64::from(sb.inode_count) {
            let node = self.inode(number)?;
            if !node.is_used() {
                continue;
            }
            if node.kind == KIND_FILE && node.links != 1 {
                return Err(Error::Corrupt);
            }
            // `stage_write` allocates exactly one block per sector of content
            // and zeroes every slot beyond the last.  A missing block makes the
            // file unreadable (`read_file` returns Corrupt on a zero slot); an
            // extra block beyond the file size is a resource leak that no
            // runtime path reclaims.  Both are inconsistent with the write
            // invariant, so flag them here.
            if node.kind == KIND_FILE {
                let needed = (node.size as usize).div_ceil(SECTOR_SIZE);
                for slot in 0..DIRECT_BLOCKS {
                    if (node.direct[slot] != 0) != (slot < needed) {
                        return Err(Error::Corrupt);
                    }
                }
            }
            // `read_file` verifies file content against `data_checksum` on
            // every read, so `check()` must catch the same mismatch.  A file
            // whose data blocks were silently overwritten on disk (with the
            // inode checksum left intact) would pass every structural check
            // above but fail when read — the oracle-vs-runtime gap this
            // closes.  The checksum is computed the same way as in
            // `stage_write`: FNV-1a over the raw content bytes, chained
            // block by block through `fnv1a_seeded` so no allocation is
            // needed.  Size 0 has no blocks to read and no checksum to verify
            // (the runtime creates 0-size files with a zeroed checksum field
            // via `stage_create`, not `stage_write`).
            if node.kind == KIND_FILE && node.size > 0 {
                let size = node.size as usize;
                let needed = size.div_ceil(SECTOR_SIZE);
                let mut hash = 0xcbf2_9ce4_8422_2325_u64;
                for slot in 0..needed {
                    let block = node.direct[slot];
                    let sector = self.read_block(block)?;
                    let tail = if slot + 1 == needed && !size.is_multiple_of(SECTOR_SIZE) {
                        size % SECTOR_SIZE
                    } else {
                        SECTOR_SIZE
                    };
                    hash = fnv1a_seeded(hash, &sector[..tail]);
                }
                if hash != node.data_checksum {
                    return Err(Error::Corrupt);
                }
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
            }
            // No other live inode may name any of this inode's blocks. A block
            // repeated within one inode is already refused by `checked_inode`
            // (via `self.inode` above); here we reuse the write path's
            // `refuse_shared_blocks` so the verifier shares one definition of
            // the cross-inode rule. The former fixed-size `seen` set stopped
            // recording after 64 distinct blocks, so an alias involving any
            // later block went unseen and this verifier could call a corrupt
            // image clean; this scan has no such bound.
            self.refuse_shared_blocks(&txn, number, &node.direct)?;
        }

        // Every data block the bitmap marks allocated must be named by exactly
        // one live inode.  The per-inode loop above already checks the converse:
        // every inode-named block has its bitmap bit set.  A block the bitmap
        // calls allocated that no live inode names is a leaked resource — the
        // allocator will never hand it out again, so the space is permanently
        // lost and the bitmap disagrees with the inode table.
        for block in sb.data_start..sb.total_blocks {
            if !self.bitmap_get(&txn, block)? {
                continue;
            }
            let mut named = false;
            'search: for number in 1..=u64::from(sb.inode_count) {
                let node = self.inode(number)?;
                if node.is_used() && node.direct.contains(&block) {
                    named = true;
                    break 'search;
                }
            }
            if !named {
                return Err(Error::Corrupt);
            }
        }

        let root = self.inode(sb.root_inode)?;
        if root.kind != KIND_DIR {
            return Err(Error::Corrupt);
        }
        // `format()` sets the root directory's link count to 2 (one from the
        // filesystem root, one from "." inside it). No runtime operation ever
        // modifies a directory's link count, so any value other than 2 is
        // corruption that `format()` cannot have produced.
        if root.links != 2 {
            return Err(Error::Corrupt);
        }
        let view = self.dir_entries(&txn, &root)?;
        for entry in view.entries.iter().filter(|entry| entry.inode != 0) {
            if entry.inode > u64::from(sb.inode_count) {
                return Err(Error::Corrupt);
            }
            let node = self.inode(entry.inode)?;
            // Every entry must name a live file inode and carry KIND_FILE in its
            // own kind field. `stage_create` always writes KIND_FILE for both,
            // so any deviation is corruption.
            if !node.is_used() || node.kind != KIND_FILE || entry.kind != KIND_FILE {
                return Err(Error::Corrupt);
            }
            // `stage_create` always sets name_len to the actual name length, which
            // `checked_vfs_name` bounds to 1..=MAX_NAME. Zero or any value above
            // MAX_NAME is corruption; `matches()` would silently skip the entry.
            if entry.name_len == 0 || usize::from(entry.name_len) > MAX_NAME {
                return Err(Error::Corrupt);
            }
            // `checked_native_name` rejects names with null bytes; Rust `&str`
            // cannot carry them, so a stored null byte makes the entry permanently
            // unlookupable. Any null byte within name[0..name_len] is corruption.
            let name_len = usize::from(entry.name_len);
            if entry.name[..name_len].contains(&0u8) {
                return Err(Error::Corrupt);
            }
            // `stage_create` rejects names containing '/'; all VFS paths go
            // through `checked_native_name` which also rejects it. An entry
            // whose name bytes include '/' can never be reached by lookup or
            // unlink, making it permanently orphaned corruption.
            if entry.name[..name_len].contains(&b'/') {
                return Err(Error::Corrupt);
            }
            // `checked_native_name` also rejects backslash ('\\').  An entry
            // whose name bytes include '\\' can never be reached by lookup or
            // unlink for the same reason, making it permanently orphaned
            // corruption.
            if entry.name[..name_len].contains(&b'\\') {
                return Err(Error::Corrupt);
            }
            // `checked_native_name` rejects "." and ".." as directory-navigation
            // names that are never valid file names.  A stored entry with either
            // name can never be created through the normal path; any such entry
            // is corruption.
            if entry.name[..name_len] == b"."[..] || entry.name[..name_len] == b".."[..] {
                return Err(Error::Corrupt);
            }
        }
        // No two live entries may share the same name. `stage_create` rejects
        // duplicates, so any duplicate pair is corruption; the second entry
        // can never be reached by `lookup` or `unlink`.
        for i in 0..view.entries.len() {
            if view.entries[i].inode == 0 {
                continue;
            }
            let ni = usize::from(view.entries[i].name_len);
            for j in 0..i {
                if view.entries[j].inode == 0 {
                    continue;
                }
                let nj = usize::from(view.entries[j].name_len);
                if ni == nj && view.entries[i].name[..ni] == view.entries[j].name[..nj] {
                    return Err(Error::Corrupt);
                }
            }
        }
        // No two live entries may name the same inode. `stage_create` always
        // allocates a fresh inode, so any two entries sharing an inode number
        // represent an impossible hard link inconsistent with `links == 1`.
        // After unlinking one entry, the other would become a stale reference
        // to a now-free inode — the corrupt state the earlier checks detect.
        for i in 0..view.entries.len() {
            if view.entries[i].inode == 0 {
                continue;
            }
            for j in 0..i {
                if view.entries[j].inode == 0 {
                    continue;
                }
                if view.entries[i].inode == view.entries[j].inode {
                    return Err(Error::Corrupt);
                }
            }
        }
        // Every live KIND_FILE inode must be reachable via exactly one
        // directory entry. An orphaned inode — live (mode != 0), kind ==
        // KIND_FILE, but no directory entry names it — represents a permanent
        // space leak: `unlink` works by name, so neither the inode slot nor
        // its data blocks can ever be freed through the normal path. The bitmap
        // direction alone cannot surface this: the orphan's blocks are
        // correctly marked allocated and named by the inode, so both bitmap
        // checks pass. Only the inode↔entry cross-reference exposes it.
        for number in 1..=u64::from(sb.inode_count) {
            let node = self.inode(number)?;
            if !node.is_used() || node.kind != KIND_FILE {
                continue;
            }
            if !view.entries.iter().any(|entry| entry.inode == number) {
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

    #[cfg(any(test, feature = "stage15-1-test", feature = "stage15-2-test"))]
    pub fn live(&self) -> impl Iterator<Item = &DirEntry> {
        self.entries.iter().filter(|entry| entry.inode != 0)
    }

    #[cfg(any(test, feature = "stage15-1-test", feature = "stage15-2-test"))]
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

    // Verify overwrite persistence through a fresh mount, with checksum checks.
    let Ok(mut before_overwrite) = Volume::mount(base.fork(None)) else {
        return false;
    };
    let Ok(file_id) = before_overwrite.create(1, "overwrite.bin", 0o600, 50) else {
        return false;
    };
    if before_overwrite
        .write_file(file_id, b"old version", 51)
        .is_err()
    {
        return false;
    }
    let disk_before = before_overwrite.into_device();
    let Ok(mut overwriter) = Volume::mount(disk_before.fork(None)) else {
        return false;
    };
    if overwriter.write_file(file_id, b"new version", 52).is_err() {
        return false;
    }
    let disk_after = overwriter.into_device();
    let Ok(mut recovered_overwrite) = Volume::mount(disk_after.fork(None)) else {
        return false;
    };
    let mut overwritten = [0u8; 32];
    if recovered_overwrite.read_file(file_id, &mut overwritten) != Ok(11)
        || &overwritten[..11] != b"new version"
        || recovered_overwrite.check().is_err()
    {
        return false;
    }

    // Exercise every interrupted sector write of an overwrite transaction.
    // After journal replay, a file must contain either the entire old payload
    // or the entire new payload, never a partially overwritten mixture.
    let Ok(mut prepared) = Volume::mount(base.fork(None)) else {
        return false;
    };
    let Ok(target) = prepared.create(1, "atomic-write.bin", 0o600, 60) else {
        return false;
    };
    let old_bytes = [0x11u8; 700];
    let new_bytes = [0x22u8; 700];
    if prepared.write_file(target, &old_bytes, 61).is_err() {
        return false;
    }
    let old_disk = prepared.into_device();
    let writes = {
        let Ok(mut measured) = Volume::mount(old_disk.fork(None)) else {
            return false;
        };
        if measured.write_file(target, &new_bytes, 62).is_err() {
            return false;
        }
        measured.into_device().accepted()
    };
    for boundary in 0..=writes {
        let Ok(mut interrupted) = Volume::mount(old_disk.fork(Some(boundary))) else {
            return false;
        };
        let _ = interrupted.write_file(target, &new_bytes, 62);
        let interrupted_disk = interrupted.into_device();
        let Ok(mut recovered) = Volume::mount(interrupted_disk.fork(None)) else {
            return false;
        };
        let mut bytes = [0u8; 700];
        if recovered.check().is_err()
            || recovered.read_file(target, &mut bytes) != Ok(bytes.len())
            || (bytes != old_bytes && bytes != new_bytes)
        {
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

// ------------------------------------------------------- real device mount

/// VFS mount point for the native filesystem.
#[cfg(feature = "stage15-2-test")]
pub const MOUNT_POINT: &str = "/wfs";

/// Outcome of probing and mounting the ATA primary master as WovenFS.
#[cfg(feature = "stage15-2-test")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MountOutcome {
    /// No ATA primary master is present.
    NoDevice,
    /// The device carries no valid WovenFS superblock.
    NotWovenFs,
    /// Mounted; the value is how many files were imported into the VFS.
    Mounted(usize),
    /// A device or consistency error prevented mounting.
    Failed,
}

// WovenFS mutations are read-modify-write transactions. Serialize the whole
// mounted operation so concurrent tasks cannot commit lost updates. Rank 5
// remains below the ATA device lock (rank 10).
#[cfg(not(test))]
static MUTATION: crate::irq_lock::PreemptMutex<()> =
    crate::irq_lock::PreemptMutex::with_rank((), 5);

/// Run `operation` against the ATA primary master mounted as WovenFS.
///
/// The volume is mounted for the duration of the call and dropped afterwards,
/// so no stale in-memory filesystem state outlives the operation. Mounting
/// replays any committed journal transaction, which is why every entry point
/// goes through here rather than caching a volume.
#[cfg(not(test))]
pub fn with_ata_volume<R>(
    operation: impl FnOnce(&mut Volume<&mut crate::ata::BufferedAta>) -> R,
) -> Option<R> {
    crate::ata::with_primary_master(|disk| {
        let mut volume = Volume::mount(disk).ok()?;
        Some(operation(&mut volume))
    })
    .flatten()
}

/// Translate the mounted VFS namespace into one native WovenFS root name.
/// The adapter intentionally does not accept nested paths, traversal, or
/// prefix-confusion paths such as `/wfs2/...`.
#[cfg(not(test))]
fn checked_vfs_name(path: &str) -> Result<&str, Error> {
    let name = path.strip_prefix("/wfs/").ok_or(Error::InvalidName)?;
    checked_native_name(name)
}

/// Ensure a VFS-visible file exists durably before publishing its VFS node.
#[cfg(not(test))]
pub fn ensure_vfs_path(path: &str, now: u64) -> Result<(), Error> {
    let _mutation = MUTATION.lock();
    let name = checked_vfs_name(path)?;
    with_ata_volume(|volume| match volume.lookup(1, name) {
        Ok(entry) if entry.kind == KIND_FILE => Ok(()),
        Ok(_) => Err(Error::NotADirectory),
        Err(Error::NotFound) => volume.create(1, name, 0o600, now).map(|_| ()),
        Err(error) => Err(error),
    })
    .unwrap_or(Err(Error::Device))
}

/// Commit a native write and publish its VFS cache image while the native
/// mutation lock is still held. This prevents a later native writer from
/// committing and publishing before an earlier writer publishes its cache.
/// A publication error still follows a durable commit: callers must reconcile
/// the cache on failure rather than assume the native write was rolled back.
#[cfg(not(test))]
pub fn write_vfs_path_at_with_publish<T, E>(
    path: &str,
    offset: usize,
    bytes: &[u8],
    out: &mut [u8; MAX_FILE_SIZE],
    now: u64,
    publish: impl FnOnce(&[u8], usize, usize) -> Result<T, E>,
) -> Result<Result<T, E>, Error> {
    let _mutation = MUTATION.lock();
    let name = checked_vfs_name(path)?;
    // Reject impossible offsets before creating a new inode on disk.
    // Otherwise an invalid write could leave an empty file behind.
    if offset > MAX_FILE_SIZE || bytes.len() > MAX_FILE_SIZE.saturating_sub(offset) {
        return Err(Error::TooLarge);
    }
    let (written, length) = with_ata_volume(|volume| {
        let inode = match volume.lookup(1, name) {
            Ok(entry) => entry.inode,
            // Create and fill a missing file in one transaction, so a failed
            // write cannot leave an empty file behind.
            Err(Error::NotFound) if offset == 0 => {
                volume.create_file(1, name, 0o600, bytes, now)?;
                out[..bytes.len()].copy_from_slice(bytes);
                return Ok((bytes.len(), bytes.len()));
            }
            // WovenFS does not support sparse writes. Do not create a file
            // for an offset that cannot be valid for a new empty inode.
            Err(Error::NotFound) => return Err(Error::TooLarge),
            Err(error) => return Err(error),
        };
        let mut length = volume.read_file(inode, out)?;
        if offset > length || bytes.len() > MAX_FILE_SIZE.saturating_sub(offset) {
            return Err(Error::TooLarge);
        }
        let end = offset + bytes.len();
        out[offset..end].copy_from_slice(bytes);
        length = length.max(end);
        volume.write_file(inode, &out[..length], now)?;
        Ok((bytes.len(), length))
    })
    .unwrap_or(Err(Error::Device))?;
    Ok(publish(&out[..length], length, written))
}

/// Remove a single native root file. The VFS entry is removed by the caller
/// only after this durable transaction succeeds.
#[cfg(not(test))]
pub fn unlink_vfs_path(path: &str, now: u64) -> Result<(), Error> {
    let _mutation = MUTATION.lock();
    let name = checked_vfs_name(path)?;
    with_ata_volume(|volume| {
        let entry = volume.lookup(1, name)?;
        if entry.kind != KIND_FILE {
            return Err(Error::NotADirectory);
        }
        volume.unlink(1, name, now)
    })
    .unwrap_or(Err(Error::Device))
}

#[cfg(test)]
pub fn write_vfs_path_at_with_publish<T, E>(
    _path: &str,
    _offset: usize,
    _bytes: &[u8],
    _out: &mut [u8; MAX_FILE_SIZE],
    _now: u64,
    _publish: impl FnOnce(&[u8], usize, usize) -> Result<T, E>,
) -> Result<Result<T, E>, Error> {
    Err(Error::Device)
}

#[cfg(test)]
pub fn ensure_vfs_path(_path: &str, _now: u64) -> Result<(), Error> {
    Err(Error::Device)
}

#[cfg(test)]
pub fn unlink_vfs_path(_path: &str, _now: u64) -> Result<(), Error> {
    Err(Error::Device)
}

/// Explicit authority for native WovenFS mutations. Must be issued only after
/// a WovenGuard capability check by the kernel-side caller.
#[cfg(feature = "stage15-2-test")]
pub struct AuthorizedWfsWriter {
    _private: (),
}

#[cfg(feature = "stage15-2-test")]
impl AuthorizedWfsWriter {
    pub(crate) const fn after_capability_check() -> Self {
        Self { _private: () }
    }
}

/// Validate the native root-directory filename before any ATA I/O. Shared
/// by the native and VFS-path adapters, so both refuse the same names.
#[cfg(not(test))]
fn checked_native_name(name: &str) -> Result<&str, Error> {
    if name.is_empty()
        || name.len() > MAX_NAME
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\\')
        || name.as_bytes().contains(&0)
    {
        return Err(Error::InvalidName);
    }
    Ok(name)
}

/// Native write-through adapter. It remounts and replays the journal for each
/// operation; VFS namespace invalidation remains a separate integration gate.
#[cfg(feature = "stage15-2-test")]
pub fn write_ata_file(
    _authority: &AuthorizedWfsWriter,
    name: &str,
    bytes: &[u8],
    now: u64,
) -> Result<(), Error> {
    checked_native_name(name)?;
    if bytes.len() > MAX_FILE_SIZE {
        return Err(Error::TooLarge);
    }
    with_ata_volume(|volume| match volume.lookup(1, name) {
        Ok(entry) => volume.write_file(entry.inode, bytes, now),
        Err(Error::NotFound) => volume.create_file(1, name, 0o600, bytes, now).map(|_| ()),
        Err(error) => Err(error),
    })
    .unwrap_or(Err(Error::Device))
}

/// Reads newly mounted, checksum-verified disk content, not cached VFS bytes.
#[cfg(feature = "stage15-2-test")]
pub fn read_ata_file(name: &str, out: &mut [u8]) -> Result<usize, Error> {
    checked_native_name(name)?;
    with_ata_volume(|volume| {
        let inode = volume.lookup(1, name)?.inode;
        volume.read_file(inode, out)
    })
    .unwrap_or(Err(Error::Device))
}

/// Authorized native unlink. Callers must invalidate VFS cached entries.
#[cfg(feature = "stage15-2-test")]
pub fn unlink_ata_file(
    _authority: &AuthorizedWfsWriter,
    name: &str,
    now: u64,
) -> Result<(), Error> {
    checked_native_name(name)?;
    with_ata_volume(|volume| volume.unlink(1, name, now)).unwrap_or(Err(Error::Device))
}

/// Explicit VFS-path adapter: commits to the native journal before reporting
/// success. No VFS RAM copy is modified, avoiding false durability claims.
/// This is NOT yet hooked into generic VFS descriptor/syscall dispatch.
#[cfg(feature = "stage15-2-test")]
pub fn vfs_write_through(
    authority: &AuthorizedWfsWriter,
    path: &str,
    bytes: &[u8],
    now: u64,
) -> Result<(), Error> {
    write_ata_file(authority, checked_vfs_name(path)?, bytes, now)
}

/// Read directly from the mounted volume with checksum verification.
#[cfg(feature = "stage15-2-test")]
pub fn vfs_read_through(path: &str, out: &mut [u8]) -> Result<usize, Error> {
    read_ata_file(checked_vfs_name(path)?, out)
}

/// Delete a file using the same bounded namespace and explicit authority.
#[cfg(feature = "stage15-2-test")]
pub fn vfs_unlink_through(
    authority: &AuthorizedWfsWriter,
    path: &str,
    now: u64,
) -> Result<(), Error> {
    unlink_ata_file(authority, checked_vfs_name(path)?, now)
}

/// Format the ATA primary master as WovenFS.
///
/// Destructive, and gated behind the Stage 15.2 acceptance feature so it can
/// only run in a test boot against a scratch disk.
#[cfg(feature = "stage15-2-test")]
pub fn format_ata(inode_count: u32) -> bool {
    crate::ata::with_primary_master(|disk| match Volume::format(disk, inode_count) {
        Ok(_) => true,
        Err(error) => {
            // Name the error. On real hardware a bare false is not diagnosable,
            // and the serial log is the only evidence available.
            crate::serial::write_line(format_args!("[S15.2] format error: {:?}", error));
            false
        }
    })
    .unwrap_or(false)
}

/// Probe the ATA primary master, and on success publish its root directory
/// into the VFS under [`MOUNT_POINT`].
///
/// Files are imported read-only. Write-back from the VFS into WovenFS is a
/// later stage; until it exists, presenting these as writable would imply a
/// durability path that is not there.
#[cfg(feature = "stage15-2-test")]
pub fn mount_ata_into_vfs() -> MountOutcome {
    let Some(result) = crate::ata::with_primary_master(|disk| {
        let mut volume = match Volume::mount(disk) {
            Ok(volume) => volume,
            Err(Error::Corrupt) => return MountOutcome::NotWovenFs,
            Err(_) => return MountOutcome::Failed,
        };
        if volume.check().is_err() {
            return MountOutcome::Failed;
        }
        let root = volume.super_block().root_inode;
        let view = match volume.read_dir(root) {
            Ok(view) => view,
            Err(_) => return MountOutcome::Failed,
        };

        let _ = crate::vfs::mkdir(MOUNT_POINT);
        let mut imported = 0usize;
        let mut buffer = [0u8; MAX_FILE_SIZE];
        for entry in view.entries.iter().filter(|entry| entry.inode != 0) {
            let name = entry.name_str();
            if name.is_empty() || name.len() > MAX_NAME {
                continue;
            }
            let mut path = [0u8; crate::config::MAX_PATH_SIZE];
            let prefix = MOUNT_POINT.as_bytes();
            let needed = prefix.len() + 1 + name.len();
            if needed > path.len() {
                continue;
            }
            path[..prefix.len()].copy_from_slice(prefix);
            path[prefix.len()] = b'/';
            path[prefix.len() + 1..needed].copy_from_slice(name.as_bytes());
            let Ok(path) = core::str::from_utf8(&path[..needed]) else {
                continue;
            };

            let Ok(length) = volume.read_file(entry.inode, &mut buffer) else {
                // A content checksum failure must not be imported as if the
                // bytes were good.
                continue;
            };
            if length > crate::vfs::NODE_CAPACITY {
                continue;
            }
            match crate::vfs::create_read_only(path, &buffer[..length]) {
                Ok(()) => imported += 1,
                // A pre-existing VFS node may contain stale bytes from an
                // earlier mount. Do not count it as freshly imported: the
                // namespace needs explicit invalidation before remount.
                Err(crate::vfs::Error::AlreadyExists) => {}
                Err(_) => {}
            }
        }
        MountOutcome::Mounted(imported)
    }) else {
        return MountOutcome::NoDevice;
    };
    result
}

/// Stage 15.2 acceptance: WovenFS on a real block device, through the ATA
/// driver, visible in the VFS.
#[cfg(feature = "stage15-2-test")]
/// Stage 15.2 acceptance: WovenFS on a real block device, through the ATA
/// driver, visible in the VFS.
///
/// Each step reports its own marker. A bare pass/fail is not diagnosable on
/// real hardware, where a failure can come from the device rather than the
/// filesystem, and the serial log is the only evidence available.
#[cfg(feature = "stage15-2-test")]
pub fn stage15_2_self_test() -> bool {
    const INODES: u32 = 32;
    let alpha = b"wovenfs-alpha-contents";
    let beta: [u8; 700] = {
        let mut bytes = [0u8; 700];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = (index % 97) as u8;
        }
        bytes
    };

    fn step(name: &str, ok: bool) -> bool {
        crate::serial::write_line(format_args!(
            "[S15.2] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }

    // Report the geometry first: on an unfamiliar device this is what explains
    // every later decision.
    let Some(sectors) = crate::ata::with_primary_master(|disk| {
        use crate::block::BlockDevice;
        disk.sector_count()
    }) else {
        crate::serial::write_line(format_args!("[S15.2] ata primary master absent FAIL"));
        return false;
    };
    crate::serial::write_line(format_args!("[S15.2] device sectors {}", sectors));

    // Before judging `format`, establish that the device accepts a write at
    // all. On real hardware a format failure is ambiguous: it could be the
    // filesystem or a device that silently refuses writes, and the two need
    // different fixes. LBA 1 is the alternate superblock slot, which `format`
    // zeroes anyway, so using it as scratch destroys nothing.
    let writable = crate::ata::with_primary_master(|disk| {
        use crate::block::BlockDevice;
        let mut probe = [0u8; SECTOR_SIZE];
        probe[..8].copy_from_slice(b"WFSPROBE");
        if let Err(error) = disk.write_sector(1, &probe) {
            crate::serial::write_line(format_args!(
                "[S15.2] sector write rejected: {}",
                match error {
                    BlockError::OutOfBounds => "OutOfBounds",
                    BlockError::InvalidBuffer => "InvalidBuffer",
                    BlockError::ReadOnly => "ReadOnly",
                    BlockError::DeviceFault => "DeviceFault",
                }
            ));
            return false;
        }
        if disk.flush().is_err() {
            crate::serial::write_line(format_args!("[S15.2] device flush rejected"));
            return false;
        }
        let mut back = [0u8; SECTOR_SIZE];
        if disk.read_sector(1, &mut back).is_err() {
            crate::serial::write_line(format_args!("[S15.2] sector read rejected"));
            return false;
        }
        if back[..8] != *b"WFSPROBE" {
            crate::serial::write_line(format_args!(
                "[S15.2] readback mismatch: {:02x} {:02x} {:02x} {:02x}",
                back[0], back[1], back[2], back[3]
            ));
            return false;
        }
        true
    });
    if !step("device accepts a sector write", writable == Some(true)) {
        return false;
    }

    // A device that carries no WovenFS superblock must not present itself as a
    // filesystem. Distinguish the outcomes so a device fault is not mistaken
    // for a clean "not WovenFS".
    let probe = mount_ata_into_vfs();
    let probe_ok = probe == MountOutcome::NotWovenFs;
    if !probe_ok {
        crate::serial::write_line(format_args!(
            "[S15.2] unformatted probe returned {}",
            match probe {
                MountOutcome::NoDevice => "NoDevice",
                MountOutcome::NotWovenFs => "NotWovenFs",
                MountOutcome::Mounted(_) => "Mounted",
                MountOutcome::Failed => "Failed",
            }
        ));
    }
    if !step("unformatted disk is not a filesystem", probe_ok) {
        return false;
    }

    if !step("format device", format_ata(INODES)) {
        return false;
    }

    let created = with_ata_volume(|volume| {
        let Ok(a) = volume.create(1, "alpha.txt", 0o644, 1) else {
            return false;
        };
        if volume.write_file(a, alpha, 2).is_err() {
            return false;
        }
        let Ok(b) = volume.create(1, "beta.bin", 0o644, 3) else {
            return false;
        };
        volume.write_file(b, &beta, 4).is_ok()
    });
    if !step("create and write two files", created == Some(true)) {
        return false;
    }

    // Exercise the authorized adapter across distinct mount lifetimes.
    // The same ATA device is reopened for each operation, so the read must
    // observe committed disk state rather than a retained Volume instance.
    let authority = AuthorizedWfsWriter::after_capability_check();
    let write_through = write_ata_file(&authority, "adapter.txt", b"persistent-v1", 20).is_ok();
    let mut adapter_buf = [0u8; 64];
    let read_through = read_ata_file("adapter.txt", &mut adapter_buf)
        .map(|n| &adapter_buf[..n] == b"persistent-v1")
        .unwrap_or(false);
    let overwritten = write_ata_file(&authority, "adapter.txt", b"v2", 21).is_ok();
    let mut overwritten_buf = [0u8; 64];
    let reread = read_ata_file("adapter.txt", &mut overwritten_buf)
        .map(|n| &overwritten_buf[..n] == b"v2")
        .unwrap_or(false);
    let removed = unlink_ata_file(&authority, "adapter.txt", 22).is_ok()
        && read_ata_file("adapter.txt", &mut overwritten_buf) == Err(Error::NotFound);
    if !step(
        "authorized write-through across remounts",
        write_through && read_through && overwritten && reread && removed,
    ) {
        return false;
    }

    // The VFS-path boundary must not accept sibling prefixes or traversal.
    let authority = AuthorizedWfsWriter::after_capability_check();
    let mut path_bytes = [0u8; 32];
    let vfs_adapter_ok = vfs_write_through(&authority, "/wfs/vfs-test.txt", b"vfs-durable", 25)
        .is_ok()
        && vfs_read_through("/wfs/vfs-test.txt", &mut path_bytes) == Ok(11)
        && &path_bytes[..11] == b"vfs-durable"
        && vfs_write_through(&authority, "/wfs2/escape", b"bad", 26) == Err(Error::InvalidName)
        && vfs_write_through(&authority, "/wfs/../escape", b"bad", 26) == Err(Error::InvalidName)
        && vfs_unlink_through(&authority, "/wfs/vfs-test.txt", 27).is_ok()
        && vfs_read_through("/wfs/vfs-test.txt", &mut path_bytes) == Err(Error::NotFound);
    if !step(
        "VFS path write-through and namespace isolation",
        vfs_adapter_ok,
    ) {
        return false;
    }

    // Invalid native names must fail before device access, regardless of
    // whether the caller reaches the filesystem through a VFS path.
    let invalid_native_names = ["", ".", "..", "nested/file", "nested\\file", "nul\0file"];
    let native_boundary_ok = invalid_native_names.iter().all(|name| {
        write_ata_file(&authority, name, b"no", 28) == Err(Error::InvalidName)
            && read_ata_file(name, &mut path_bytes) == Err(Error::InvalidName)
            && unlink_ata_file(&authority, name, 28) == Err(Error::InvalidName)
    });
    if !step("native WovenFS filename validation", native_boundary_ok) {
        return false;
    }
    let invalid_vfs_paths = [
        "/wfs",
        "/wfs/",
        "/wfs2/file",
        "/wfs/../file",
        "/wfs/sub/file",
        "/wfs/nul\0file",
    ];
    let vfs_boundary_ok = invalid_vfs_paths.iter().all(|path| {
        vfs_write_through(&authority, path, b"no", 29) == Err(Error::InvalidName)
            && vfs_read_through(path, &mut path_bytes) == Err(Error::InvalidName)
            && vfs_unlink_through(&authority, path, 29) == Err(Error::InvalidName)
    });
    if !step(
        "VFS adapter rejects invalid paths for every operation",
        vfs_boundary_ok,
    ) {
        return false;
    }

    // Reject oversized mutations and overlong names before touching ATA.
    // A rejected overwrite must preserve the previously committed bytes.
    let too_large = [0u8; MAX_FILE_SIZE + 1];
    let oversized_rejected =
        vfs_write_through(&authority, "/wfs/alpha.txt", &too_large, 30) == Err(Error::TooLarge);
    let long_name = "x".repeat(MAX_NAME + 1);
    let long_path = alloc::format!("/wfs/{long_name}");
    let overlong_rejected =
        vfs_write_through(&authority, &long_path, b"no", 31) == Err(Error::InvalidName);
    let mut durable_after_rejection = [0u8; 64];
    let original_preserved = vfs_read_through("/wfs/alpha.txt", &mut durable_after_rejection)
        .map(|n| &durable_after_rejection[..n] == alpha)
        .unwrap_or(false);
    if !step(
        "oversized and overlong mutations preserve durable data",
        oversized_rejected && overlong_rejected && original_preserved,
    ) {
        return false;
    }

    let verified = with_ata_volume(|volume| {
        if volume.check().is_err() {
            return false;
        }
        let Ok(a) = volume.lookup(1, "alpha.txt") else {
            return false;
        };
        let Ok(b) = volume.lookup(1, "beta.bin") else {
            return false;
        };
        let mut out = [0u8; MAX_FILE_SIZE];
        if volume.read_file(a.inode, &mut out) != Ok(alpha.len()) || out[..alpha.len()] != *alpha {
            return false;
        }
        if volume.read_file(b.inode, &mut out) != Ok(beta.len()) || out[..beta.len()] != beta {
            return false;
        }
        volume.read_dir(1).map(|view| view.count()) == Ok(2)
    });
    if !step("remount and verify contents", verified == Some(true)) {
        return false;
    }

    let mounted = mount_ata_into_vfs();
    if !step("mount into the VFS", mounted == MountOutcome::Mounted(2)) {
        crate::serial::write_line(format_args!(
            "[S15.2] vfs mount imported {}",
            match mounted {
                MountOutcome::Mounted(count) => count,
                _ => 0,
            }
        ));
        return false;
    }

    let mut vfs_buffer = [0u8; MAX_FILE_SIZE];
    let vfs_read = crate::vfs::read_all("/wfs/alpha.txt", &mut vfs_buffer) == Ok(alpha.len())
        && vfs_buffer[..alpha.len()] == *alpha
        && crate::vfs::read_all("/wfs/beta.bin", &mut vfs_buffer) == Ok(beta.len())
        && vfs_buffer[..beta.len()] == beta;
    if !step("read both files through the VFS", vfs_read) {
        return false;
    }

    // VFS imports are read-only until an authorized durable backend is installed.
    // Rejecting mutation prevents memory-only writes from masquerading as durable IO.
    let denied = crate::vfs::write_file("/wfs/alpha.txt", b"unauthorized-change").is_err();
    if !step("imported VFS file rejects write-through", denied) {
        return false;
    }

    // A denied VFS write must not alter the durable device contents.
    let unchanged = with_ata_volume(|volume| {
        let Ok(entry) = volume.lookup(1, "alpha.txt") else {
            return false;
        };
        let mut persisted = [0u8; MAX_FILE_SIZE];
        volume.read_file(entry.inode, &mut persisted) == Ok(alpha.len())
            && persisted[..alpha.len()] == *alpha
    });
    if !step(
        "denied VFS write preserves durable contents",
        unchanged == Some(true),
    ) {
        return false;
    }

    let removed = with_ata_volume(|volume| {
        volume.unlink(1, "alpha.txt", 5).is_ok()
            && volume.lookup(1, "alpha.txt") == Err(Error::NotFound)
            && volume.check().is_ok()
    });
    if !step("unlink on the device", removed == Some(true)) {
        return false;
    }

    let persisted = with_ata_volume(|volume| {
        volume.lookup(1, "alpha.txt") == Err(Error::NotFound)
            && volume.lookup(1, "beta.bin").is_ok()
    });
    step("unlink survives a remount", persisted == Some(true))
}
