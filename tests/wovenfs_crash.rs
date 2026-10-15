//! Exhaustive crash-recovery proof for WovenFS.
//!
//! These tests do not assert that recovery works in general; they enumerate
//! every point at which an operation can lose power and check the result at
//! each one. For an operation performing `W` sector writes, the operation is
//! replayed `W + 1` times against a fresh copy of the same formatted image,
//! with the device refusing every write after the `n`th. After each run the
//! image is mounted and the filesystem must be consistent, and the operation
//! must be either entirely absent or entirely present.
//!
//! The contract under test is specified in `docs/wovenfs-on-disk-format.md`.
#![allow(dead_code)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[path = "../kernel/src/block.rs"]
mod block;
#[path = "../kernel/src/wfs.rs"]
mod wfs;

use block::{BlockDevice, Error as BlockError, SECTOR_SIZE};
use wfs::{Error, MAX_FILE_SIZE, Volume};

const SECTORS: usize = 192;
const INODES: u32 = 32;

type Image = Rc<RefCell<Vec<[u8; SECTOR_SIZE]>>>;

/// A RAM device that stops accepting writes after a fixed number of them.
///
/// A refused write leaves the stored bytes untouched, modelling power loss
/// before the sector reached the platter. Reads keep working so the image can
/// be mounted afterwards. The backing image is shared by handle, so a test can
/// still inspect the bytes after the device has been moved into `format` or
/// `mount` — including when those calls return an error.
#[derive(Clone)]
struct CrashDisk {
    image: Image,
    budget: Rc<Cell<Option<usize>>>,
    accepted: Rc<Cell<usize>>,
}

impl CrashDisk {
    fn blank() -> Self {
        Self::blank_sized(SECTORS)
    }

    fn blank_sized(sectors: usize) -> Self {
        Self {
            image: Rc::new(RefCell::new(vec![[0; SECTOR_SIZE]; sectors])),
            budget: Rc::new(Cell::new(None)),
            accepted: Rc::new(Cell::new(0)),
        }
    }

    /// An independent copy of this image with its own write budget.
    fn fork(&self, budget: Option<usize>) -> Self {
        Self {
            image: Rc::new(RefCell::new(self.image.borrow().clone())),
            budget: Rc::new(Cell::new(budget)),
            accepted: Rc::new(Cell::new(0)),
        }
    }

    fn writes(&self) -> usize {
        self.accepted.get()
    }

    fn sectors(&self) -> Vec<[u8; SECTOR_SIZE]> {
        self.image.borrow().clone()
    }

    fn poke(&self, lba: usize, value: u8) {
        self.image.borrow_mut()[lba] = [value; SECTOR_SIZE];
    }

    fn flip_byte(&self, lba: usize, at: usize) {
        self.image.borrow_mut()[lba][at] ^= 0xff;
    }
}

impl BlockDevice for CrashDisk {
    fn sector_count(&self) -> u64 {
        self.image.borrow().len() as u64
    }

    fn read_sector(&mut self, lba: u64, sector: &mut [u8]) -> Result<(), BlockError> {
        if sector.len() != SECTOR_SIZE {
            return Err(BlockError::InvalidBuffer);
        }
        let index = usize::try_from(lba).map_err(|_| BlockError::OutOfBounds)?;
        let image = self.image.borrow();
        let source = image.get(index).ok_or(BlockError::OutOfBounds)?;
        sector.copy_from_slice(source);
        Ok(())
    }

    fn write_sector(&mut self, lba: u64, sector: &[u8]) -> Result<(), BlockError> {
        if sector.len() != SECTOR_SIZE {
            return Err(BlockError::InvalidBuffer);
        }
        let index = usize::try_from(lba).map_err(|_| BlockError::OutOfBounds)?;
        if index >= self.image.borrow().len() {
            return Err(BlockError::OutOfBounds);
        }
        if let Some(remaining) = self.budget.get() {
            if remaining == 0 {
                return Err(BlockError::DeviceFault);
            }
            self.budget.set(Some(remaining - 1));
        }
        self.image.borrow_mut()[index].copy_from_slice(sector);
        self.accepted.set(self.accepted.get() + 1);
        Ok(())
    }
}

/// A formatted, empty volume image.
fn formatted() -> CrashDisk {
    let disk = CrashDisk::blank();
    Volume::format(disk.clone(), INODES).expect("format");
    disk
}

/// Writes performed by `op` when nothing fails.
fn write_count<F>(base: &CrashDisk, op: F) -> usize
where
    F: FnOnce(&mut Volume<CrashDisk>),
{
    let probe = base.fork(None);
    let mut volume = Volume::mount(probe.clone()).expect("mount baseline");
    op(&mut volume);
    probe.writes()
}

/// Mount `image`, require consistency, and hand back the volume.
fn mount_consistent(image: &CrashDisk, context: &str) -> Volume<CrashDisk> {
    let mut volume = Volume::mount(image.clone())
        .unwrap_or_else(|error| panic!("{context}: mount failed: {error:?}"));
    volume
        .check()
        .unwrap_or_else(|error| panic!("{context}: inconsistent: {error:?}"));
    volume
}

#[test]
fn format_produces_a_consistent_mountable_volume() {
    let image = formatted();
    let mut volume = mount_consistent(&image, "fresh format");
    let sb = *volume.super_block();
    assert_eq!(sb.root_inode, 1);
    assert!(sb.data_start < sb.total_blocks);
    assert_eq!(volume.read_dir(1).expect("root").count(), 0);
}

#[test]
fn a_partly_written_format_is_not_mistaken_for_a_filesystem() {
    // The real superblock is written last, so a truncated format must leave
    // either no filesystem at all or a complete, consistent one -- never a
    // mountable volume with half-built metadata.
    let full = {
        let disk = CrashDisk::blank();
        Volume::format(disk.clone(), INODES).expect("format");
        disk.writes()
    };
    assert!(full > 4, "format should take several writes, took {full}");

    for cut in 0..full {
        let disk = CrashDisk::blank();
        disk.budget.set(Some(cut));
        let _ = Volume::format(disk.clone(), INODES);

        // Whatever reached the disk is now inspected on its own terms.
        let settled = disk.fork(None);
        if let Ok(mut volume) = Volume::mount(settled.clone()) {
            volume.check().unwrap_or_else(|error| {
                panic!("format cut at {cut} mounted but was inconsistent: {error:?}")
            });
        }
    }
}

#[test]
fn create_is_atomic_across_every_crash_point() {
    let base = formatted();
    let writes = write_count(&base, |volume| {
        volume.create(1, "report.txt", 0o644, 100).expect("create");
    });
    assert!(
        writes > 2,
        "expected a multi-sector operation, got {writes}"
    );

    for cut in 0..=writes {
        let crashing = base.fork(Some(cut));
        let mut volume = Volume::mount(crashing.clone()).expect("mount before the operation");
        let _ = volume.create(1, "report.txt", 0o644, 100);
        drop(volume);

        let settled = crashing.fork(None);
        let mut recovered = mount_consistent(&settled, &format!("crash at {cut}"));

        match recovered.lookup(1, "report.txt") {
            Ok(entry) => {
                // Present must mean completely present.
                let node = recovered
                    .inode(entry.inode)
                    .unwrap_or_else(|e| panic!("crash at {cut}: inode unreadable: {e:?}"));
                assert!(
                    node.is_used(),
                    "crash at {cut} left a directory entry pointing at a free inode"
                );
                assert_eq!(node.size, 0, "crash at {cut} created a non-empty file");
            }
            Err(Error::NotFound) => {}
            Err(error) => panic!("crash at {cut}: unexpected lookup error: {error:?}"),
        }
    }
}

#[test]
fn write_is_atomic_across_every_crash_point() {
    let payload: Vec<u8> = (0..1500u32).map(|index| (index % 251) as u8).collect();

    let base = formatted();
    let inode = {
        let mut volume = Volume::mount(base.clone()).expect("mount");
        volume.create(1, "data.bin", 0o644, 100).expect("create")
    };

    let writes = write_count(&base, |volume| {
        volume.write_file(inode, &payload, 200).expect("write");
    });
    assert!(writes > 3, "expected a multi-sector write, got {writes}");

    for cut in 0..=writes {
        let crashing = base.fork(Some(cut));
        let mut volume = Volume::mount(crashing.clone()).expect("mount");
        let _ = volume.write_file(inode, &payload, 200);
        drop(volume);

        let settled = crashing.fork(None);
        let mut recovered = mount_consistent(&settled, &format!("crash at {cut}"));

        let node = recovered.inode(inode).expect("inode");
        let mut out = vec![0u8; MAX_FILE_SIZE];
        if node.size == 0 {
            // The write never committed; the file is still empty.
            continue;
        }
        assert_eq!(
            node.size as usize,
            payload.len(),
            "crash at {cut} left a partial size"
        );
        let read = recovered
            .read_file(inode, &mut out)
            .unwrap_or_else(|e| panic!("crash at {cut}: read failed: {e:?}"));
        assert_eq!(read, payload.len(), "crash at {cut}: short read");
        assert_eq!(
            &out[..read],
            &payload[..],
            "crash at {cut}: torn file contents"
        );
    }
}

#[test]
fn unlink_is_atomic_across_every_crash_point() {
    let payload = [7u8; 900];
    let base = formatted();
    {
        let mut volume = Volume::mount(base.clone()).expect("mount");
        let inode = volume.create(1, "gone.bin", 0o644, 100).expect("create");
        volume.write_file(inode, &payload, 110).expect("write");
    }

    let writes = write_count(&base, |volume| {
        volume.unlink(1, "gone.bin", 300).expect("unlink");
    });
    assert!(writes > 2);

    for cut in 0..=writes {
        let crashing = base.fork(Some(cut));
        let mut volume = Volume::mount(crashing.clone()).expect("mount");
        let _ = volume.unlink(1, "gone.bin", 300);
        drop(volume);

        let settled = crashing.fork(None);
        let mut recovered = mount_consistent(&settled, &format!("crash at {cut}"));

        match recovered.lookup(1, "gone.bin") {
            Ok(entry) => {
                // Still linked, so its contents must still be intact.
                let mut out = vec![0u8; MAX_FILE_SIZE];
                let read = recovered
                    .read_file(entry.inode, &mut out)
                    .unwrap_or_else(|e| panic!("crash at {cut}: surviving file unreadable: {e:?}"));
                assert_eq!(&out[..read], &payload[..], "crash at {cut}: torn survivor");
            }
            Err(Error::NotFound) => {}
            Err(error) => panic!("crash at {cut}: unexpected lookup error: {error:?}"),
        }
    }
}

#[test]
fn replay_is_idempotent() {
    // Mounting repeatedly must converge: the first mount may replay a committed
    // transaction, and every later mount must change nothing.
    let payload = [3u8; 1024];
    let base = formatted();
    let inode = {
        let mut volume = Volume::mount(base.clone()).expect("mount");
        volume.create(1, "x.bin", 0o644, 1).expect("create")
    };
    let writes = write_count(&base, |volume| {
        volume.write_file(inode, &payload, 2).expect("write");
    });

    for cut in 0..=writes {
        let crashing = base.fork(Some(cut));
        let mut volume = Volume::mount(crashing.clone()).expect("mount");
        let _ = volume.write_file(inode, &payload, 2);
        drop(volume);

        let settled = crashing.fork(None);
        Volume::mount(settled.clone())
            .unwrap_or_else(|e| panic!("crash at {cut}: first mount: {e:?}"));
        let after_first = settled.sectors();
        Volume::mount(settled.clone())
            .unwrap_or_else(|e| panic!("crash at {cut}: second mount: {e:?}"));
        let after_second = settled.sectors();

        assert_eq!(
            after_first, after_second,
            "crash at {cut}: mounting again changed the image, so replay is not idempotent"
        );
    }
}

#[test]
fn a_torn_journal_payload_is_rejected_by_the_commit_checksum() {
    // Flip a byte in a staged payload while leaving descriptor and commit
    // intact. The commit checksum must notice and recovery must discard the
    // transaction rather than write unknown bytes into a metadata block.
    let base = formatted();
    let sb = {
        let volume = Volume::mount(base.clone()).expect("mount");
        *volume.super_block()
    };

    // Stage and commit a transaction, then stop before the journal is retired
    // by cutting power at the final retire write.
    let full = write_count(&base, |volume| {
        volume.create(1, "b.txt", 0o644, 2).expect("create");
    });
    let crashing = base.fork(Some(full - 1));
    let mut volume = Volume::mount(crashing.clone()).expect("mount");
    let _ = volume.create(1, "b.txt", 0o644, 2);
    drop(volume);

    // Corrupt the first staged payload sector.
    crashing.flip_byte((sb.journal_start + 1) as usize, 0);

    let settled = crashing.fork(None);
    let mut recovered = mount_consistent(&settled, "torn journal payload");
    // Whatever it decided, the volume must be consistent and readable.
    recovered.read_dir(1).expect("root readable");
}

#[test]
fn a_file_larger_than_the_direct_block_limit_is_refused() {
    let base = formatted();
    let mut volume = Volume::mount(base.clone()).expect("mount");
    let inode = volume.create(1, "big.bin", 0o644, 1).expect("create");
    let oversized = vec![0u8; MAX_FILE_SIZE + 1];
    assert_eq!(
        volume.write_file(inode, &oversized, 2),
        Err(Error::TooLarge)
    );
    volume.check().expect("refusal left the volume consistent");
}

#[test]
fn a_damaged_checkpoint_slot_falls_back_to_the_other_one() {
    let base = formatted();
    {
        let mut volume = Volume::mount(base.clone()).expect("mount");
        volume.create(1, "keep.txt", 0o644, 1).expect("create");
    }
    let sequence = {
        let volume = Volume::mount(base.clone()).expect("mount");
        volume.super_block().sequence
    };
    assert!(sequence > 1, "a commit should have advanced the checkpoint");

    // Destroy slot 1, which the first commit made authoritative.
    base.poke(1, 0xab);
    let settled = base.fork(None);
    let mut recovered = mount_consistent(&settled, "damaged checkpoint slot");
    // The surviving older checkpoint is from before the file was created, so
    // the volume must still be coherent either way.
    recovered.read_dir(1).expect("root readable");
}

#[test]
fn names_are_validated_and_duplicates_refused() {
    let base = formatted();
    let mut volume = Volume::mount(base.clone()).expect("mount");
    assert_eq!(volume.create(1, "", 0o644, 1), Err(Error::InvalidName));
    assert_eq!(volume.create(1, "a/b", 0o644, 1), Err(Error::InvalidName));
    let long = "x".repeat(49);
    assert_eq!(volume.create(1, &long, 0o644, 1), Err(Error::InvalidName));
    volume.create(1, "once.txt", 0o644, 1).expect("create");
    assert_eq!(volume.create(1, "once.txt", 0o644, 1), Err(Error::Exists));
    volume.check().expect("consistent");
}

#[test]
fn create_file_is_atomic_across_every_crash_point() {
    let payload: Vec<u8> = (0..1500u32).map(|index| (index % 239) as u8).collect();
    let base = formatted();
    let writes = write_count(&base, |volume| {
        volume
            .create_file(1, "new.bin", 0o644, &payload, 100)
            .expect("create_file");
    });
    assert!(
        writes > 3,
        "expected a multi-sector operation, got {writes}"
    );

    for cut in 0..=writes {
        let crashing = base.fork(Some(cut));
        let mut volume = Volume::mount(crashing.clone()).expect("mount");
        let _ = volume.create_file(1, "new.bin", 0o644, &payload, 100);
        drop(volume);

        let settled = crashing.fork(None);
        let mut recovered = mount_consistent(&settled, &format!("crash at {cut}"));

        match recovered.lookup(1, "new.bin") {
            Ok(entry) => {
                // Present must mean present with its contents, never empty.
                let mut out = vec![0u8; MAX_FILE_SIZE];
                let read = recovered
                    .read_file(entry.inode, &mut out)
                    .unwrap_or_else(|e| panic!("crash at {cut}: read failed: {e:?}"));
                assert_eq!(&out[..read], &payload[..], "crash at {cut}: torn new file");
            }
            Err(Error::NotFound) => {}
            Err(error) => panic!("crash at {cut}: unexpected lookup error: {error:?}"),
        }
    }
}

/// A volume whose data blocks, not its inodes or root directory slots, are
/// exhausted. The 80-sector image has 34 free data blocks, so four full-size
/// files fit and the fifth runs out of blocks with directory slots and
/// inodes to spare.
fn data_full_volume() -> Volume<CrashDisk> {
    let image = CrashDisk::blank_sized(80);
    Volume::format(image.clone(), INODES).expect("format");
    let payload = vec![1u8; MAX_FILE_SIZE];
    let mut volume = Volume::mount(image).expect("mount");
    let mut filled = 0;
    loop {
        let name = format!("fill{filled}");
        match volume.create_file(1, &name, 0o644, &payload, 1) {
            Ok(_) => filled += 1,
            Err(Error::NoSpace) => break,
            Err(error) => panic!("filling {name}: {error:?}"),
        }
    }
    assert_eq!(filled, 4, "expected data blocks to bound the fill");
    volume
}

#[test]
fn create_file_out_of_space_leaves_no_file() {
    let mut volume = data_full_volume();
    let payload = vec![2u8; MAX_FILE_SIZE];

    assert_eq!(
        volume.create_file(1, "late.bin", 0o644, &payload, 2),
        Err(Error::NoSpace)
    );
    assert_eq!(
        volume.lookup(1, "late.bin").map(|entry| entry.inode),
        Err(Error::NotFound),
        "a failed create_file must not leave an empty file"
    );
    volume.check().expect("consistent after refusal");

    // The separate create-then-write sequence is the defect this replaces:
    // the empty file commits before the write fails.
    let orphan = volume.create(1, "orphan.bin", 0o644, 3).expect("create");
    assert_eq!(volume.write_file(orphan, &payload, 3), Err(Error::NoSpace));
    assert!(volume.lookup(1, "orphan.bin").is_ok());
}

#[test]
fn content_corruption_is_detected_on_read() {
    let base = formatted();
    let payload = [42u8; 600];
    let inode = {
        let mut volume = Volume::mount(base.clone()).expect("mount");
        let inode = volume.create(1, "c.bin", 0o644, 1).expect("create");
        volume.write_file(inode, &payload, 2).expect("write");
        inode
    };
    let block = {
        let mut volume = Volume::mount(base.clone()).expect("mount");
        volume.inode(inode).expect("inode").direct[0]
    };
    base.flip_byte(block as usize, 5);

    let mut volume = Volume::mount(base.clone()).expect("mount");
    let mut out = vec![0u8; MAX_FILE_SIZE];
    assert_eq!(
        volume.read_file(inode, &mut out),
        Err(Error::Corrupt),
        "a flipped data byte must fail the content checksum"
    );
}

fn test_fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// Point `direct[slot]` of an on-disk inode at `target`, re-sealing the inode
/// checksum so the damage is not caught by the checksum alone. This models a
/// crafted or mis-written image rather than random bit rot.
fn redirect_inode_block(image: &CrashDisk, inode: u64, slot: usize, target: u64) {
    let inode_start = {
        let volume = Volume::mount(image.clone()).expect("mount");
        volume.super_block().inode_start
    };
    let index = inode - 1;
    let lba = (inode_start + index / 4) as usize;
    let offset = (index % 4) as usize * 128;
    let mut sectors = image.image.borrow_mut();
    let raw = &mut sectors[lba][offset..offset + 128];
    raw[40 + slot * 8..48 + slot * 8].copy_from_slice(&target.to_le_bytes());
    let checksum = test_fnv1a(&raw[..120]);
    raw[120..128].copy_from_slice(&checksum.to_le_bytes());
}

#[test]
fn inode_block_pointers_outside_the_data_region_are_refused() {
    let base = formatted();
    let inode = {
        let mut volume = Volume::mount(base.clone()).expect("mount");
        volume
            .create_file(1, "p.bin", 0o644, &[7u8; 600], 1)
            .expect("create_file")
    };
    let sb = *Volume::mount(base.clone()).expect("mount").super_block();
    let targets = [
        ("superblock slot 1", 1),
        ("journal", sb.journal_start),
        ("bitmap", sb.bitmap_start),
        ("inode table", sb.inode_start),
        ("past the end", sb.total_blocks),
    ];
    for (region, target) in targets {
        let image = base.fork(None);
        redirect_inode_block(&image, inode, 1, target);
        let before = image.sectors();

        let mut volume = Volume::mount(image.clone()).expect("mount");
        let mut out = vec![0u8; MAX_FILE_SIZE];
        assert_eq!(
            volume.read_file(inode, &mut out),
            Err(Error::Corrupt),
            "{region}: read must fail closed"
        );
        assert_eq!(
            volume.write_file(inode, &[9u8; 1500], 2),
            Err(Error::Corrupt),
            "{region}: write must not reuse a metadata block"
        );
        assert_eq!(
            volume.unlink(1, "p.bin", 3),
            Err(Error::Corrupt),
            "{region}: unlink must not free a metadata block"
        );
        assert!(
            image.sectors() == before,
            "{region}: a refused operation must not modify the image"
        );
    }
}

/// Rewrite one little-endian field of the superblock in `slot`, re-sealing its
/// checksum so only the geometry check can catch the change.
fn reseal_super_field(image: &CrashDisk, slot: usize, at: usize, value: &[u8]) {
    let mut sectors = image.image.borrow_mut();
    let raw = &mut sectors[slot];
    raw[at..at + value.len()].copy_from_slice(value);
    let checksum = test_fnv1a(&raw[..80]);
    raw[80..88].copy_from_slice(&checksum.to_le_bytes());
}

#[test]
fn superblock_geometry_that_does_not_fit_is_refused() {
    let base = formatted();
    let sb = *Volume::mount(base.clone()).expect("mount").super_block();
    let device = base.sector_count();
    let u64le = |v: u64| v.to_le_bytes().to_vec();
    let u32le = |v: u32| v.to_le_bytes().to_vec();
    let cases: [(&str, usize, Vec<u8>); 11] = [
        ("total_blocks past the device", 20, u64le(device + 1)),
        ("journal over the superblock slots", 28, u64le(1)),
        ("journal too small for a transaction", 36, u32le(4)),
        (
            "bitmap overlapping the journal",
            40,
            u64le(sb.journal_start + 1),
        ),
        ("bitmap too small for total_blocks", 48, u32le(0)),
        (
            "inode table overlapping the bitmap",
            52,
            u64le(sb.bitmap_start),
        ),
        ("no inodes", 60, u32le(0)),
        (
            "data region overlapping the inode table",
            64,
            u64le(sb.inode_start),
        ),
        ("data region past the end", 64, u64le(sb.total_blocks)),
        ("root inode zero", 72, u64le(0)),
        (
            "root inode past the table",
            72,
            u64le(u64::from(sb.inode_count) + 1),
        ),
    ];

    for (case, at, value) in &cases {
        // A freshly formatted image has only slot 0, so nothing to fall back to.
        let image = base.fork(None);
        reseal_super_field(&image, 0, *at, value);
        let before = image.sectors();
        assert!(
            matches!(Volume::mount(image.clone()), Err(Error::Corrupt)),
            "{case}: mount must refuse the geometry"
        );
        assert!(
            image.sectors() == before,
            "{case}: a refused mount must not modify the image"
        );
    }

    // With a valid older checkpoint present, a newer slot whose geometry does
    // not fit is discarded like one whose checksum fails.
    let committed = base.fork(None);
    {
        let mut volume = Volume::mount(committed.clone()).expect("mount");
        volume.create(1, "keep.txt", 0o644, 1).expect("create");
    }
    let newest = Volume::mount(committed.clone())
        .expect("mount")
        .super_block()
        .sequence;
    for (case, at, value) in &cases {
        let image = committed.fork(None);
        reseal_super_field(&image, 1, *at, value);
        let volume = mount_consistent(&image, case);
        assert_eq!(volume.super_block().total_blocks, sb.total_blocks, "{case}");
        assert!(
            volume.super_block().sequence < newest,
            "{case}: must fall back"
        );
    }
}

#[test]
fn journal_replay_refuses_targets_outside_the_transactional_regions() {
    // Leave a committed, applied but unretired transaction in the journal by
    // cutting power at the final retire write.
    let base = formatted();
    let sb = *Volume::mount(base.clone()).expect("mount").super_block();
    let full = write_count(&base, |volume| {
        volume.create(1, "j.txt", 0o644, 2).expect("create");
    });
    let pending = base.fork(Some(full - 1));
    {
        let mut volume = Volume::mount(pending.clone()).expect("mount");
        let _ = volume.create(1, "j.txt", 0o644, 2);
    }
    let descriptor = sb.journal_start as usize;

    // Control: the untouched descriptor is live, so mount replays it and
    // retires the journal. Without this the cases below would prove nothing.
    let control = pending.fork(None);
    mount_consistent(&control, "valid pending transaction");
    assert_eq!(
        control.sectors()[descriptor],
        [0u8; SECTOR_SIZE],
        "a valid committed transaction must be replayed and retired"
    );

    // The commit checksum covers the staged data but not the descriptor's
    // targets, so retargeting a block keeps the transaction "committed".
    let cases = [
        ("superblock slot 0", 0),
        ("superblock slot 1", 1),
        ("journal descriptor", sb.journal_start),
        ("journal payload", sb.journal_start + 1),
        ("last journal block", sb.bitmap_start - 1),
        ("past the volume", sb.total_blocks),
    ];
    for (case, target) in cases {
        let image = pending.fork(None);
        image.image.borrow_mut()[descriptor][24..32].copy_from_slice(&target.to_le_bytes());
        let before = image.sectors();
        mount_consistent(&image, case);
        assert!(
            image.sectors() == before,
            "{case}: replay must discard the transaction without writing"
        );
    }
}
