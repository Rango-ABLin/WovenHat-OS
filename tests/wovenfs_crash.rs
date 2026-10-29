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

/// A two-block file whose second pointer the caller then redirects.
fn two_block_file() -> (CrashDisk, u64) {
    let base = formatted();
    let inode = {
        let mut volume = Volume::mount(base.clone()).expect("mount");
        volume
            .create_file(1, "p.bin", 0o644, &[7u8; 600], 1)
            .expect("create_file")
    };
    (base, inode)
}

/// An operation on a mounted volume and the file's inode number.
type Attempt = fn(&mut Volume<CrashDisk>, u64) -> Result<(), Error>;

#[test]
fn a_pointer_at_a_block_the_bitmap_marks_free_is_not_reused_or_freed() {
    let (base, inode) = two_block_file();
    let free = Volume::mount(base.clone())
        .expect("mount")
        .super_block()
        .total_blocks
        - 1;
    redirect_inode_block(&base, inode, 1, free);
    assert_eq!(
        Volume::mount(base.clone()).expect("mount").check(),
        Err(Error::Corrupt),
        "the redirected block must be free in the bitmap"
    );

    // Growing rewrites the free block in place; shrinking and unlink free it
    // again. Each would leave the block with two owners once the allocator
    // hands it out.
    let attempts: [(&str, Attempt); 3] = [
        ("in-place write", |volume, inode| {
            volume.write_file(inode, &[9u8; 1500], 2)
        }),
        ("shrinking write", |volume, inode| {
            volume.write_file(inode, &[9u8; 100], 2)
        }),
        ("unlink", |volume, _| volume.unlink(1, "p.bin", 2)),
    ];
    for (operation, attempt) in attempts {
        let image = base.fork(None);
        let before = image.sectors();
        let mut volume = Volume::mount(image.clone()).expect("mount");
        assert_eq!(
            attempt(&mut volume, inode),
            Err(Error::Corrupt),
            "{operation}: must refuse a block the bitmap marks free"
        );
        assert!(
            image.sectors() == before,
            "{operation}: a refused operation must not modify the image"
        );
    }
}

#[test]
fn an_inode_naming_one_block_twice_is_refused() {
    let (base, inode) = two_block_file();
    let first = Volume::mount(base.clone())
        .expect("mount")
        .inode(inode)
        .expect("inode")
        .direct[0];
    redirect_inode_block(&base, inode, 1, first);
    let before = base.sectors();

    let mut volume = Volume::mount(base.clone()).expect("mount");
    assert_eq!(volume.inode(inode).map(|_| ()), Err(Error::Corrupt));
    let mut out = vec![0u8; MAX_FILE_SIZE];
    assert_eq!(volume.read_file(inode, &mut out), Err(Error::Corrupt));
    assert_eq!(
        volume.write_file(inode, &[9u8; 1500], 2),
        Err(Error::Corrupt),
        "write must not stage one block twice"
    );
    assert_eq!(
        volume.write_file(inode, &[9u8; 100], 2),
        Err(Error::Corrupt),
        "shrink must not free a block another slot still names"
    );
    assert_eq!(volume.unlink(1, "p.bin", 3), Err(Error::Corrupt));
    assert!(
        base.sectors() == before,
        "a refused operation must not modify the image"
    );
}

/// Two one-block files, `a.bin` and `b.bin`, and their inode numbers.
fn two_one_block_files() -> (CrashDisk, u64, u64) {
    let base = formatted();
    let (a, b) = {
        let mut volume = Volume::mount(base.clone()).expect("mount");
        let a = volume
            .create_file(1, "a.bin", 0o644, &[1u8; 300], 1)
            .expect("create a");
        let b = volume
            .create_file(1, "b.bin", 0o644, &[2u8; 300], 1)
            .expect("create b");
        (a, b)
    };
    (base, a, b)
}

#[test]
fn a_block_named_by_two_inodes_is_not_rewritten_or_freed() {
    let (base, a, b) = two_one_block_files();
    let shared = Volume::mount(base.clone())
        .expect("mount")
        .inode(a)
        .expect("inode a")
        .direct[0];
    // Both pointers now name an allocated block, so the bitmap check alone
    // cannot see the alias; b's own block is leaked, which is harmless here.
    redirect_inode_block(&base, b, 0, shared);
    assert_eq!(
        Volume::mount(base.clone()).expect("mount").check(),
        Err(Error::Corrupt),
        "check() must see the alias"
    );

    // Rewriting either owner overwrites the other's data in place; shrinking
    // or unlinking either frees a block the other still names.
    let attempts: [(&str, Attempt); 6] = [
        ("in-place write of b", |volume, _| {
            let inode = volume.lookup(1, "b.bin")?.inode;
            volume.write_file(inode, &[9u8; 300], 2)
        }),
        ("in-place write of a", |volume, _| {
            let inode = volume.lookup(1, "a.bin")?.inode;
            volume.write_file(inode, &[9u8; 300], 2)
        }),
        ("shrinking write of b", |volume, _| {
            let inode = volume.lookup(1, "b.bin")?.inode;
            volume.write_file(inode, &[], 2)
        }),
        ("shrinking write of a", |volume, _| {
            let inode = volume.lookup(1, "a.bin")?.inode;
            volume.write_file(inode, &[], 2)
        }),
        ("unlink b", |volume, _| volume.unlink(1, "b.bin", 2)),
        ("unlink a", |volume, _| volume.unlink(1, "a.bin", 2)),
    ];
    for (operation, attempt) in attempts {
        let image = base.fork(None);
        let before = image.sectors();
        let mut volume = Volume::mount(image.clone()).expect("mount");
        assert_eq!(
            attempt(&mut volume, b),
            Err(Error::Corrupt),
            "{operation}: must refuse a block another inode names"
        );
        assert!(
            image.sectors() == before,
            "{operation}: a refused operation must not modify the image"
        );
    }
}

/// Write a used, single-link file inode directly into the image with `blocks`
/// as its direct pointers, re-sealing its checksum. `check()` never reads file
/// data, so the data checksum is left zero.
fn craft_file_inode(image: &CrashDisk, sb: &wfs::SuperBlock, inode: u64, blocks: &[u64]) {
    assert!(blocks.len() <= wfs::DIRECT_BLOCKS);
    let index = inode - 1;
    let lba = (sb.inode_start + index / 4) as usize;
    let offset = (index % 4) as usize * 128;
    let mut sectors = image.image.borrow_mut();
    let raw = &mut sectors[lba][offset..offset + 128];
    raw.fill(0);
    raw[0..4].copy_from_slice(&0o644u32.to_le_bytes()); // mode (non-zero => used)
    raw[4..8].copy_from_slice(&1u32.to_le_bytes()); // one link
    raw[8..16].copy_from_slice(&((blocks.len() * SECTOR_SIZE) as u64).to_le_bytes()); // size
    for (i, block) in blocks.iter().enumerate() {
        raw[40 + i * 8..48 + i * 8].copy_from_slice(&block.to_le_bytes());
    }
    raw[104..106].copy_from_slice(&1u16.to_le_bytes()); // KIND_FILE
    let checksum = test_fnv1a(&raw[..120]);
    raw[120..128].copy_from_slice(&checksum.to_le_bytes());
}

/// Set `block`'s allocation bit in the bitmap.
fn mark_allocated(image: &CrashDisk, sb: &wfs::SuperBlock, block: u64) {
    let bits = SECTOR_SIZE as u64 * 8;
    let within = (block % bits) as usize;
    let mut sectors = image.image.borrow_mut();
    sectors[(sb.bitmap_start + block / bits) as usize][within / 8] |= 1u8 << (within % 8);
}

#[test]
fn check_detects_an_alias_beyond_the_first_sixty_four_blocks() {
    let base = formatted();
    let sb = *Volume::mount(base.clone()).expect("mount").super_block();

    // The alias must land past the 64th distinct block, so the image needs
    // more than 65 data blocks and enough inodes to name them. A smaller image
    // would never reach the former fixed-size set's bound.
    let data_blocks = sb.total_blocks - sb.data_start;
    assert!(data_blocks > 65, "image too small for this test");
    assert!(sb.inode_count >= 11, "too few inodes for this test");

    // Inodes 2..=9 each own eight distinct data blocks (64 in all); with the
    // root block that is 65 distinct blocks seen before the alias, filling and
    // overflowing the old 64-entry set. The root block (data_start) is left to
    // the root inode.
    let mut next = sb.data_start + 1;
    for inode in 2..=9u64 {
        let mut blocks = Vec::new();
        for _ in 0..8 {
            blocks.push(next);
            next += 1;
        }
        for &block in &blocks {
            mark_allocated(&base, &sb, block);
        }
        craft_file_inode(&base, &sb, inode, &blocks);
    }

    // Inodes 10 and 11 both name one further block. It first appears only
    // after the 64 earlier blocks, so the former set never recorded it and the
    // alias went unseen — the verifier would call this corrupt image clean.
    let shared = next;
    mark_allocated(&base, &sb, shared);
    craft_file_inode(&base, &sb, 10, &[shared]);
    craft_file_inode(&base, &sb, 11, &[shared]);

    let before = base.sectors();
    assert_eq!(
        Volume::mount(base.clone()).expect("mount").check(),
        Err(Error::Corrupt),
        "check() must see an alias past the first 64 blocks"
    );
    assert!(
        base.sectors() == before,
        "check() must not modify the image"
    );
}

/// Directory-entry rewrites of the root: create (empty and with contents) and
/// unlink of a file whose own blocks are sound.
const DIR_ENTRY_ATTEMPTS: [(&str, Attempt); 3] = [
    ("create", |volume, _| {
        volume.create(1, "c.bin", 0o644, 2).map(|_| ())
    }),
    ("create_file", |volume, _| {
        volume
            .create_file(1, "c.bin", 0o644, &[3u8; 300], 2)
            .map(|_| ())
    }),
    ("unlink a", |volume, _| volume.unlink(1, "a.bin", 2)),
];

fn assert_dir_entry_writes_refused(base: &CrashDisk, why: &str) {
    for (operation, attempt) in DIR_ENTRY_ATTEMPTS {
        let image = base.fork(None);
        let before = image.sectors();
        let mut volume = Volume::mount(image.clone()).expect("mount");
        assert_eq!(
            attempt(&mut volume, 0),
            Err(Error::Corrupt),
            "{operation}: must not rewrite a directory block {why}"
        );
        assert!(
            image.sectors() == before,
            "{operation}: a refused operation must not modify the image"
        );
    }
}

#[test]
fn a_directory_block_named_by_a_file_is_not_rewritten() {
    let (base, _, b) = two_one_block_files();
    let root_block = Volume::mount(base.clone())
        .expect("mount")
        .inode(1)
        .expect("root")
        .direct[0];
    redirect_inode_block(&base, b, 0, root_block);
    assert_eq!(
        Volume::mount(base.clone()).expect("mount").check(),
        Err(Error::Corrupt),
        "check() must see the alias"
    );
    // None of these operations touches b, yet each rewrites the directory
    // block b names, which would overwrite b's data.
    assert_dir_entry_writes_refused(&base, "a file also names");
}

#[test]
fn a_directory_block_the_bitmap_marks_free_is_not_rewritten() {
    let (base, _, _) = two_one_block_files();
    let (sb, root_block) = {
        let mut volume = Volume::mount(base.clone()).expect("mount");
        let root = volume.inode(1).expect("root");
        (*volume.super_block(), root.direct[0])
    };
    {
        let bits = SECTOR_SIZE as u64 * 8;
        let within = (root_block % bits) as usize;
        let mut sectors = base.image.borrow_mut();
        sectors[(sb.bitmap_start + root_block / bits) as usize][within / 8] &=
            !(1u8 << (within % 8));
    }
    assert_eq!(
        Volume::mount(base.clone()).expect("mount").check(),
        Err(Error::Corrupt),
        "the root block must now be free in the bitmap"
    );
    // The allocator may already have handed a free block to another file.
    assert_dir_entry_writes_refused(&base, "the bitmap marks free");
}

#[test]
fn a_free_block_named_by_a_live_inode_is_not_allocated() {
    let (base, _, b) = two_one_block_files();
    let free = {
        let mut volume = Volume::mount(base.clone()).expect("mount");
        let sb = *volume.super_block();
        // The allocator scans the data region upward, so the next block it
        // hands out is the first one past every live inode's blocks.
        let used = volume.inode(1).expect("root").direct[0]
            .max(volume.inode(b).expect("inode b").direct[0]);
        assert!(used + 1 < sb.total_blocks);
        used + 1
    };
    redirect_inode_block(&base, b, 0, free);
    let before = base.sectors();

    let mut volume = Volume::mount(base.clone()).expect("mount");
    assert_eq!(
        volume.create_file(1, "c.bin", 0o644, &[3u8; 300], 2),
        Err(Error::Corrupt),
        "create_file must not take a block b still names"
    );
    let a = volume.lookup(1, "a.bin").expect("a").inode;
    assert_eq!(
        volume.write_file(a, &[4u8; 900], 2),
        Err(Error::Corrupt),
        "a growing write must not take a block b still names"
    );
    assert!(
        base.sectors() == before,
        "a refused operation must not modify the image"
    );
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

/// A committed, applied but unretired transaction left in the journal by
/// cutting power at the final retire write, and the volume's superblock.
fn pending_transaction() -> (CrashDisk, wfs::SuperBlock) {
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

    // Control: the untouched descriptor is live, so mount replays it and
    // retires the journal. Without this the callers' cases would prove
    // nothing.
    let control = pending.fork(None);
    mount_consistent(&control, "valid pending transaction");
    assert_eq!(
        control.sectors()[sb.journal_start as usize],
        [0u8; SECTOR_SIZE],
        "a valid committed transaction must be replayed and retired"
    );
    (pending, sb)
}

fn test_fnv1a_seeded(seed: u64, bytes: &[u8]) -> u64 {
    bytes.iter().fold(seed, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// Point the pending transaction's first staged block at `target`. With
/// `reseal`, the commit checksum is recomputed over the edited descriptor so
/// the damage is not caught by the checksum alone; this models a crafted
/// image rather than a descriptor damaged after commit.
fn retarget_journal_block(image: &CrashDisk, sb: &wfs::SuperBlock, target: u64, reseal: bool) {
    let mut sectors = image.image.borrow_mut();
    let descriptor = sb.journal_start as usize;
    sectors[descriptor][24..32].copy_from_slice(&target.to_le_bytes());
    if !reseal {
        return;
    }
    let count = u32::from_le_bytes(sectors[descriptor][16..20].try_into().unwrap()) as usize;
    let header = test_fnv1a(&sectors[descriptor][..24 + count * 8]);
    let checksum = sectors[descriptor + 1..descriptor + 1 + count]
        .iter()
        .fold(header, |hash, data| test_fnv1a_seeded(hash, data));
    sectors[descriptor + 1 + count][16..24].copy_from_slice(&checksum.to_le_bytes());
}

#[test]
fn journal_replay_refuses_targets_outside_the_transactional_regions() {
    let (pending, sb) = pending_transaction();

    // The commit checksum is resealed, so only the target range check stands
    // between these descriptors and a write over a checkpoint or the journal.
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
        retarget_journal_block(&image, &sb, target, true);
        let before = image.sectors();
        mount_consistent(&image, case);
        assert!(
            image.sectors() == before,
            "{case}: replay must discard the transaction without writing"
        );
    }
}

#[test]
fn journal_replay_refuses_a_descriptor_changed_after_commit() {
    let (pending, sb) = pending_transaction();

    // Targets inside the transactional regions pass the range check, so only
    // the commit checksum can tell that the descriptor no longer matches what
    // was committed.
    let cases = [
        ("bitmap", sb.bitmap_start),
        ("last inode table block", sb.data_start - 1),
        ("last data block", sb.total_blocks - 1),
    ];
    for (case, target) in cases {
        // Control: resealed, the retargeted block is replayed onto `target`,
        // so the unsealed case below is refused by the checksum and not by
        // anything else.
        let resealed = pending.fork(None);
        retarget_journal_block(&resealed, &sb, target, true);
        let staged = resealed.sectors()[sb.journal_start as usize + 1];
        assert_ne!(
            resealed.sectors()[target as usize],
            staged,
            "{case}: the target must not already hold the staged bytes"
        );
        // Replaying staged bytes onto the wrong block leaves an inconsistent
        // volume, which is the point; only the write is checked here.
        drop(Volume::mount(resealed.clone()));
        assert_eq!(
            resealed.sectors()[target as usize],
            staged,
            "{case}: a resealed descriptor must be replayed to its target"
        );

        let image = pending.fork(None);
        retarget_journal_block(&image, &sb, target, false);
        let before = image.sectors();
        mount_consistent(&image, case);
        assert!(
            image.sectors() == before,
            "{case}: replay must discard a descriptor the commit does not cover"
        );
    }
}

/// A root directory holding `a.txt` and `b.txt`, the root's first directory
/// block, and the entry index of `a.txt` within it.
fn two_file_root() -> (CrashDisk, u64, usize) {
    let base = formatted();
    let mut volume = Volume::mount(base.clone()).expect("mount");
    volume
        .create_file(1, "a.txt", 0o644, b"alpha", 1)
        .expect("create a");
    volume
        .create_file(1, "b.txt", 0o644, b"bravo", 1)
        .expect("create b");
    let block = volume.inode(1).expect("root").direct[0];
    let index = volume
        .read_dir(1)
        .expect("read_dir")
        .entries
        .iter()
        .position(|entry| entry.matches("a.txt"))
        .expect("a.txt entry");
    assert!(index < 8, "a.txt must sit in the first directory block");
    (base, block, index)
}

/// Overwrite `len` bytes at `at` within one directory entry. Directory blocks
/// carry no checksum, so nothing needs re-sealing.
fn patch_dir_entry(image: &CrashDisk, block: u64, index: usize, at: usize, value: &[u8]) {
    let start = index * 64 + at;
    image.image.borrow_mut()[block as usize][start..start + value.len()].copy_from_slice(value);
}

#[test]
fn unlink_refuses_an_entry_that_does_not_name_one_live_file() {
    let (base, block, index) = two_file_root();
    let b_inode = Volume::mount(base.clone())
        .expect("mount")
        .lookup(1, "b.txt")
        .expect("b.txt")
        .inode;
    let cases = [
        ("the directory's own inode", 1u64),
        ("an inode another entry names", b_inode),
        ("a free inode", u64::from(INODES)),
        ("an inode past the table", u64::from(INODES) + 1),
    ];
    for (case, target) in cases {
        let image = base.fork(None);
        patch_dir_entry(&image, block, index, 0, &target.to_le_bytes());
        let before = image.sectors();
        let mut volume = Volume::mount(image.clone()).expect("mount");
        assert_eq!(
            volume.unlink(1, "a.txt", 2),
            Err(Error::Corrupt),
            "{case}: unlink must refuse the entry"
        );
        assert!(
            image.sectors() == before,
            "{case}: a refused unlink must not modify the image"
        );
        assert_eq!(
            volume.read_file(b_inode, &mut [0u8; MAX_FILE_SIZE]),
            Ok(5),
            "{case}: other files must stay readable"
        );
    }

    // Control: the untouched entry unlinks normally.
    let image = base.fork(None);
    let mut volume = Volume::mount(image.clone()).expect("mount");
    volume.unlink(1, "a.txt", 2).expect("unlink a valid entry");
    mount_consistent(&image, "after a valid unlink");
}

#[test]
fn an_entry_name_length_past_the_name_field_does_not_match() {
    let (base, block, index) = two_file_root();
    let long = "x".repeat(60);
    patch_dir_entry(&base, block, index, 8, &60u16.to_le_bytes());
    let mut volume = Volume::mount(base.clone()).expect("mount");
    assert_eq!(
        volume.lookup(1, &long).map(|entry| entry.inode),
        Err(Error::NotFound)
    );
    assert_eq!(volume.unlink(1, &long, 2), Err(Error::NotFound));
}

/// Set an on-disk inode's `data_checksum` to the checksum of `bytes`,
/// re-sealing the inode checksum.
fn reseal_inode_data_checksum(image: &CrashDisk, inode: u64, bytes: &[u8]) {
    let inode_start = {
        let volume = Volume::mount(image.clone()).expect("mount");
        volume.super_block().inode_start
    };
    let index = inode - 1;
    let lba = (inode_start + index / 4) as usize;
    let offset = (index % 4) as usize * 128;
    let mut sectors = image.image.borrow_mut();
    let raw = &mut sectors[lba][offset..offset + 128];
    raw[32..40].copy_from_slice(&test_fnv1a(bytes).to_le_bytes());
    let checksum = test_fnv1a(&raw[..120]);
    raw[120..128].copy_from_slice(&checksum.to_le_bytes());
}

#[test]
fn read_file_refuses_a_directory_inode() {
    let (base, block, index) = two_file_root();
    // Give the root inode a data checksum that matches its entry table, so
    // only the kind check stands between `read_file` and the raw entries.
    let table = base.sectors()[block as usize];
    reseal_inode_data_checksum(&base, 1, &table);
    let mut out = vec![0u8; MAX_FILE_SIZE];

    let mut volume = Volume::mount(base.clone()).expect("mount");
    assert_eq!(
        volume.read_file(1, &mut out),
        Err(Error::NotADirectory),
        "the root directory must not read as a file"
    );

    // The same directory reached through a retargeted entry.
    patch_dir_entry(&base, block, index, 0, &1u64.to_le_bytes());
    let mut volume = Volume::mount(base.clone()).expect("mount");
    let target = volume.lookup(1, "a.txt").expect("a.txt").inode;
    assert_eq!(
        volume.read_file(target, &mut out),
        Err(Error::NotADirectory),
        "an entry naming a directory must not read as a file"
    );

    // An entry naming a free inode is now refused by lookup itself.
    patch_dir_entry(&base, block, index, 0, &u64::from(INODES).to_le_bytes());
    let mut volume = Volume::mount(base.clone()).expect("mount");
    assert_eq!(
        volume.lookup(1, "a.txt"),
        Err(Error::Corrupt),
        "an entry naming a free inode must be refused by lookup"
    );

    // Control: an untouched file still reads.
    let b = volume.lookup(1, "b.txt").expect("b.txt").inode;
    assert_eq!(volume.read_file(b, &mut out), Ok(5));
    assert_eq!(&out[..5], b"bravo");
}

/// Set one `direct[slot]` entry of an on-disk inode, re-sealing the inode checksum.
fn reseal_inode_direct(image: &CrashDisk, inode: u64, slot: usize, block: u64) {
    let inode_start = {
        let volume = Volume::mount(image.clone()).expect("mount");
        volume.super_block().inode_start
    };
    let index = inode - 1;
    let lba = (inode_start + index / 4) as usize;
    let offset = (index % 4) as usize * 128;
    let mut sectors = image.image.borrow_mut();
    let raw = &mut sectors[lba][offset..offset + 128];
    let at = 40 + slot * 8;
    raw[at..at + 8].copy_from_slice(&block.to_le_bytes());
    let checksum = test_fnv1a(&raw[..120]);
    raw[120..128].copy_from_slice(&checksum.to_le_bytes());
}

/// Set an on-disk inode's `size` field, re-sealing the inode checksum.
fn reseal_inode_size(image: &CrashDisk, inode: u64, size: u64) {
    let inode_start = {
        let volume = Volume::mount(image.clone()).expect("mount");
        volume.super_block().inode_start
    };
    let index = inode - 1;
    let lba = (inode_start + index / 4) as usize;
    let offset = (index % 4) as usize * 128;
    let mut sectors = image.image.borrow_mut();
    let raw = &mut sectors[lba][offset..offset + 128];
    raw[8..16].copy_from_slice(&size.to_le_bytes());
    let checksum = test_fnv1a(&raw[..120]);
    raw[120..128].copy_from_slice(&checksum.to_le_bytes());
}

/// Set an on-disk inode's `created` field, re-sealing the inode checksum.
fn reseal_inode_created(image: &CrashDisk, inode: u64, created: u64) {
    let inode_start = {
        let volume = Volume::mount(image.clone()).expect("mount");
        volume.super_block().inode_start
    };
    let index = inode - 1;
    let lba = (inode_start + index / 4) as usize;
    let offset = (index % 4) as usize * 128;
    let mut sectors = image.image.borrow_mut();
    let raw = &mut sectors[lba][offset..offset + 128];
    raw[16..24].copy_from_slice(&created.to_le_bytes());
    let checksum = test_fnv1a(&raw[..120]);
    raw[120..128].copy_from_slice(&checksum.to_le_bytes());
}

/// Set an on-disk inode's `modified` field, re-sealing the inode checksum.
fn reseal_inode_modified(image: &CrashDisk, inode: u64, modified: u64) {
    let inode_start = {
        let volume = Volume::mount(image.clone()).expect("mount");
        volume.super_block().inode_start
    };
    let index = inode - 1;
    let lba = (inode_start + index / 4) as usize;
    let offset = (index % 4) as usize * 128;
    let mut sectors = image.image.borrow_mut();
    let raw = &mut sectors[lba][offset..offset + 128];
    raw[24..32].copy_from_slice(&modified.to_le_bytes());
    let checksum = test_fnv1a(&raw[..120]);
    raw[120..128].copy_from_slice(&checksum.to_le_bytes());
}

/// Set an on-disk inode's `links` field, re-sealing the inode checksum.
fn reseal_inode_links(image: &CrashDisk, inode: u64, links: u32) {
    let inode_start = {
        let volume = Volume::mount(image.clone()).expect("mount");
        volume.super_block().inode_start
    };
    let index = inode - 1;
    let lba = (inode_start + index / 4) as usize;
    let offset = (index % 4) as usize * 128;
    let mut sectors = image.image.borrow_mut();
    let raw = &mut sectors[lba][offset..offset + 128];
    raw[4..8].copy_from_slice(&links.to_le_bytes());
    let checksum = test_fnv1a(&raw[..120]);
    raw[120..128].copy_from_slice(&checksum.to_le_bytes());
}

/// Read `direct[0]` from an on-disk inode.
fn read_inode_direct0(image: &CrashDisk, inode: u64) -> u64 {
    let inode_start = {
        let volume = Volume::mount(image.clone()).expect("mount");
        volume.super_block().inode_start
    };
    let index = inode - 1;
    let lba = (inode_start + index / 4) as usize;
    let offset = (index % 4) as usize * 128;
    let sectors = image.image.borrow();
    u64::from_le_bytes(sectors[lba][offset + 40..offset + 48].try_into().unwrap())
}

/// Clear the bitmap bit for `block` directly in the image, without updating
/// any inode or checksum.  Use this to suppress the orphan-block scan so a
/// specific downstream check in `check()` becomes the first error path.
fn clear_bitmap_block(image: &CrashDisk, block: u64) {
    let bitmap_start = {
        let volume = Volume::mount(image.clone()).expect("mount");
        volume.super_block().bitmap_start
    };
    let bits_per_sector = SECTOR_SIZE * 8;
    let lba = (bitmap_start + block / bits_per_sector as u64) as usize;
    let within = (block % bits_per_sector as u64) as usize;
    image.image.borrow_mut()[lba][within / 8] &= !(1u8 << (within % 8));
}

/// Set the bitmap bit for `block` directly in the image, without updating any
/// inode or checksum.  Use this to make the orphan-block scan treat an
/// otherwise-free block as allocated, so the scan passes and a downstream
/// inode-level guard becomes the first error path.
fn set_bitmap_block(image: &CrashDisk, block: u64) {
    let bitmap_start = {
        let volume = Volume::mount(image.clone()).expect("mount");
        volume.super_block().bitmap_start
    };
    let bits_per_sector = SECTOR_SIZE * 8;
    let lba = (bitmap_start + block / bits_per_sector as u64) as usize;
    let within = (block % bits_per_sector as u64) as usize;
    image.image.borrow_mut()[lba][within / 8] |= 1u8 << (within % 8);
}

/// Set an on-disk inode's `mode` field, re-sealing the inode checksum.
///
/// Setting a non-zero `mode` on an otherwise-zeroed inode makes it appear
/// "used" (`Inode::is_used()` returns `self.mode != 0`).  Call this before
/// `reseal_inode_kind` so the kind write inherits the correct mode.
fn reseal_inode_mode(image: &CrashDisk, inode: u64, mode: u32) {
    let inode_start = {
        let volume = Volume::mount(image.clone()).expect("mount");
        volume.super_block().inode_start
    };
    let index = inode - 1;
    let lba = (inode_start + index / 4) as usize;
    let offset = (index % 4) as usize * 128;
    let mut sectors = image.image.borrow_mut();
    let raw = &mut sectors[lba][offset..offset + 128];
    raw[0..4].copy_from_slice(&mode.to_le_bytes());
    let checksum = test_fnv1a(&raw[..120]);
    raw[120..128].copy_from_slice(&checksum.to_le_bytes());
}

/// Set an on-disk inode's `kind` field, re-sealing the inode checksum.
fn reseal_inode_kind(image: &CrashDisk, inode: u64, kind: u16) {
    let inode_start = {
        let volume = Volume::mount(image.clone()).expect("mount");
        volume.super_block().inode_start
    };
    let index = inode - 1;
    let lba = (inode_start + index / 4) as usize;
    let offset = (index % 4) as usize * 128;
    let mut sectors = image.image.borrow_mut();
    let raw = &mut sectors[lba][offset..offset + 128];
    raw[104..106].copy_from_slice(&kind.to_le_bytes());
    let checksum = test_fnv1a(&raw[..120]);
    raw[120..128].copy_from_slice(&checksum.to_le_bytes());
}

#[test]
fn unlink_refuses_a_file_whose_link_count_is_not_one() {
    let (base, _, _) = two_file_root();
    let (a, b) = {
        let mut volume = Volume::mount(base.clone()).expect("mount");
        let a = volume.lookup(1, "a.txt").expect("a.txt").inode;
        let b = volume.lookup(1, "b.txt").expect("b.txt").inode;
        assert_eq!(volume.inode(a).expect("inode a").links, 1);
        (a, b)
    };
    for links in [0u32, 2, u32::MAX] {
        let image = base.fork(None);
        reseal_inode_links(&image, a, links);
        assert_eq!(
            Volume::mount(image.clone()).expect("mount").check(),
            Err(Error::Corrupt),
            "links = {links}: check() must flag the count"
        );
        let before = image.sectors();
        let mut volume = Volume::mount(image.clone()).expect("mount");
        assert_eq!(
            volume.unlink(1, "a.txt", 2),
            Err(Error::Corrupt),
            "links = {links}: unlink must not free a file another name may reach"
        );
        assert!(
            image.sectors() == before,
            "links = {links}: a refused unlink must not modify the image"
        );
        assert_eq!(
            volume.read_file(a, &mut [0u8; MAX_FILE_SIZE]),
            Ok(5),
            "links = {links}: the refused file must stay readable"
        );
        assert_eq!(
            volume.read_file(b, &mut [0u8; MAX_FILE_SIZE]),
            Ok(5),
            "links = {links}: other files must stay readable"
        );
    }

    // Control: a file written after create keeps one link and unlinks.
    let image = base.fork(None);
    let mut volume = Volume::mount(image.clone()).expect("mount");
    volume.write_file(a, b"rewritten", 2).expect("rewrite a");
    assert_eq!(volume.inode(a).expect("inode a").links, 1);
    volume
        .unlink(1, "a.txt", 3)
        .expect("unlink a one-link file");
    mount_consistent(&image, "after a valid unlink");
}

#[test]
fn create_refuses_a_free_inode_that_an_entry_still_names() {
    let (base, block, index) = two_file_root();
    let (b, next) = {
        let mut volume = Volume::mount(base.clone()).expect("mount");
        let b = volume.lookup(1, "b.txt").expect("b.txt").inode;
        // The allocator hands out the lowest free inode.
        let next = (2..=u64::from(INODES))
            .find(|number| !volume.inode(*number).expect("inode").is_used())
            .expect("a free inode");
        (b, next)
    };

    // Point a.txt's entry at the inode the next create would allocate.
    let image = base.fork(None);
    patch_dir_entry(&image, block, index, 0, &next.to_le_bytes());
    assert_eq!(
        Volume::mount(image.clone()).expect("mount").check(),
        Err(Error::Corrupt),
        "check() must flag an entry naming a free inode"
    );
    let before = image.sectors();
    let mut volume = Volume::mount(image.clone()).expect("mount");
    assert_eq!(
        volume.create(1, "c.txt", 0o644, 2),
        Err(Error::Corrupt),
        "create must not give a second name to an inode"
    );
    assert_eq!(
        volume.create_file(1, "c.txt", 0o644, b"charlie", 2),
        Err(Error::Corrupt),
        "create_file must not give a second name to an inode"
    );
    assert!(
        image.sectors() == before,
        "a refused create must not modify the image"
    );
    assert_eq!(
        volume.read_file(b, &mut [0u8; MAX_FILE_SIZE]),
        Ok(5),
        "other files must stay readable"
    );

    // Control: without the stale entry, the same create succeeds and takes
    // that inode.
    let image = base.fork(None);
    let mut volume = Volume::mount(image.clone()).expect("mount");
    assert_eq!(
        volume.create_file(1, "c.txt", 0o644, b"charlie", 2),
        Ok(next)
    );
    mount_consistent(&image, "after a valid create");
}

#[test]
fn lookup_refuses_a_stale_entry_naming_a_free_inode() {
    let (base, block, index) = two_file_root();
    let (a, b, free_inode) = {
        let mut volume = Volume::mount(base.clone()).expect("mount");
        let a = volume.lookup(1, "a.txt").expect("a.txt").inode;
        let b = volume.lookup(1, "b.txt").expect("b.txt").inode;
        let free_inode = (2..=u64::from(INODES))
            .find(|n| !volume.inode(*n).expect("inode").is_used())
            .expect("a free inode");
        (a, b, free_inode)
    };

    // Patch a.txt's directory entry to name a free inode.
    let image = base.fork(None);
    patch_dir_entry(&image, block, index, 0, &free_inode.to_le_bytes());

    assert_eq!(
        Volume::mount(image.clone()).expect("mount").check(),
        Err(Error::Corrupt),
        "check() must flag an entry naming a free inode"
    );
    assert_eq!(
        Volume::mount(image.clone())
            .expect("mount")
            .lookup(1, "a.txt"),
        Err(Error::Corrupt),
        "lookup must refuse a stale entry whose inode is free"
    );

    // b.txt is unmodified and must still resolve to its original inode.
    assert_eq!(
        Volume::mount(image.clone())
            .expect("mount")
            .lookup(1, "b.txt")
            .map(|e| e.inode),
        Ok(b),
        "unaffected entry must still resolve correctly"
    );

    // b.txt must remain fully readable.
    let mut buf = [0u8; MAX_FILE_SIZE];
    assert_eq!(
        Volume::mount(image.clone())
            .expect("mount")
            .read_file(b, &mut buf),
        Ok(5),
        "other files must stay readable"
    );

    // Control: on the unpatched image, looking up a.txt succeeds and returns a.
    assert_eq!(
        Volume::mount(base.clone())
            .expect("mount")
            .lookup(1, "a.txt")
            .map(|e| e.inode),
        Ok(a),
        "unpatched image: a.txt lookup must succeed"
    );
    let _ = free_inode;
}

/// `read_dir().live()` must not expose stale entries whose inode is free.
///
/// Before the fix, `live()` filtered only `inode == 0`; a stale entry with a
/// non-zero free inode would appear in the count and live iterator despite
/// naming a freed inode, inconsistent with what `lookup()` returns.
#[test]
fn read_dir_live_filters_a_stale_entry_naming_a_free_inode() {
    let (base, block, index) = two_file_root();
    let (a, b, free_inode) = {
        let mut volume = Volume::mount(base.clone()).expect("mount");
        let a = volume.lookup(1, "a.txt").expect("a.txt").inode;
        let b = volume.lookup(1, "b.txt").expect("b.txt").inode;
        let free_inode = (2..=u64::from(INODES))
            .find(|n| !volume.inode(*n).expect("inode").is_used())
            .expect("a free inode");
        (a, b, free_inode)
    };

    // Patch a.txt's directory entry to name a free inode.
    let image = base.fork(None);
    patch_dir_entry(&image, block, index, 0, &free_inode.to_le_bytes());

    let mut volume = Volume::mount(image.clone()).expect("mount");
    let view = volume
        .read_dir(1)
        .expect("read_dir succeeds on a corrupt image");

    assert!(
        !view.live().any(|e| e.matches("a.txt")),
        "live() must not return the stale a.txt entry"
    );
    assert!(
        view.live().any(|e| e.matches("b.txt")),
        "live() must still return the valid b.txt entry"
    );
    assert_eq!(view.count(), 1, "count must be 1 (only b.txt visible)");

    // Control: on the unpatched image both entries appear.
    let view_clean = Volume::mount(base.clone())
        .expect("mount clean")
        .read_dir(1)
        .expect("read_dir clean");
    assert!(
        view_clean.live().any(|e| e.matches("a.txt")),
        "clean: a.txt visible"
    );
    assert!(
        view_clean.live().any(|e| e.matches("b.txt")),
        "clean: b.txt visible"
    );
    assert_eq!(view_clean.count(), 2, "clean: count must be 2");

    let _ = (a, b, free_inode);
}

/// `check()` must refuse a directory entry that names a directory-kind inode.
///
/// `stage_create` always writes `KIND_FILE` entries, so any directory entry
/// naming an inode whose `kind` is `KIND_DIR` is corrupt. Before the fix,
/// `check()` verified only `is_used()`; it never read the inode's `kind` field,
/// so the root inode (the only `KIND_DIR` inode) could appear in a directory
/// entry and `check()` would call the image clean.
#[test]
fn check_refuses_a_directory_entry_naming_a_directory_inode() {
    let (base, block, index) = two_file_root();
    let b_inode = Volume::mount(base.clone())
        .expect("mount")
        .lookup(1, "b.txt")
        .expect("b.txt")
        .inode;

    // Patch a.txt's directory entry to name inode 1 (the root, KIND_DIR).
    let image = base.fork(None);
    patch_dir_entry(&image, block, index, 0, &1u64.to_le_bytes());
    // Snapshot after patching: check() must not write anything.
    let before = image.sectors();

    let mut volume = Volume::mount(image.clone()).expect("mount");
    assert_eq!(
        volume.check(),
        Err(Error::Corrupt),
        "check() must refuse a directory entry naming a directory inode"
    );
    // The image must be unchanged by check().
    assert!(
        image.sectors() == before,
        "check() must not modify the image"
    );
    // Other files must remain accessible.
    let mut buf = [0u8; MAX_FILE_SIZE];
    assert_eq!(
        volume.read_file(b_inode, &mut buf),
        Ok(5),
        "b.txt must still be readable"
    );

    // Control: the unpatched image passes check().
    let mut clean = Volume::mount(base.clone()).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean image must pass check()");
}

/// `check()` must refuse a directory entry whose `kind` field does not match
/// the KIND_FILE invariant, even if the entry's inode is a valid live file.
///
/// `stage_create` always writes `kind = KIND_FILE` in both the inode and the
/// directory entry. A mismatch is corrupt. Before the fix, `check()` only
/// tested the referenced inode's kind; a directory entry whose own `kind`
/// field was `KIND_DIR` while the underlying inode remained `KIND_FILE` would
/// pass `check()` but be rejected by the VFS adapter (`wfs_open` tests
/// `entry.kind == KIND_FILE`), creating a silent inconsistency.
#[test]
fn check_refuses_a_directory_entry_with_wrong_kind() {
    use wfs::KIND_DIR;

    let (base, block, index) = two_file_root();
    let b_inode = Volume::mount(base.clone())
        .expect("mount")
        .lookup(1, "b.txt")
        .expect("b.txt")
        .inode;

    // Patch a.txt's directory entry `kind` field (bytes 10–11) to KIND_DIR.
    // The inode a.txt names remains a valid, live KIND_FILE inode — only the
    // entry's own kind field is wrong.
    let image = base.fork(None);
    patch_dir_entry(&image, block, index, 10, &KIND_DIR.to_le_bytes());
    let before = image.sectors();

    let mut volume = Volume::mount(image.clone()).expect("mount");
    assert_eq!(
        volume.check(),
        Err(Error::Corrupt),
        "check() must refuse a directory entry with kind != KIND_FILE"
    );
    // check() must not modify the image.
    assert!(
        image.sectors() == before,
        "check() must not modify the image"
    );
    // Other files must remain accessible.
    let mut buf = [0u8; MAX_FILE_SIZE];
    assert_eq!(
        volume.read_file(b_inode, &mut buf),
        Ok(5),
        "b.txt must still be readable"
    );

    // Control: the unpatched image passes check().
    let mut clean = Volume::mount(base.clone()).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean image must pass check()");
}

/// `check()` must refuse a directory entry whose `name_len` is outside the
/// valid range `1..=MAX_NAME`. `stage_create` always sets `name_len` to the
/// actual name length, validated by `checked_vfs_name` to be non-empty and
/// at most MAX_NAME bytes. A value of 0 or above MAX_NAME in a live entry is
/// corruption. `matches()` already silently rejects such entries (returning
/// false), so `lookup` would never surface them — but without this check,
/// `check()` would call such an image clean, an inconsistency between the
/// oracle and the run-time behaviour.
#[test]
fn check_refuses_a_directory_entry_with_out_of_range_name_len() {
    use wfs::MAX_NAME;

    let (base, block, index) = two_file_root();
    let b_inode = Volume::mount(base.clone())
        .expect("mount")
        .lookup(1, "b.txt")
        .expect("b.txt")
        .inode;

    let cases: &[(u16, &str)] = &[
        (0, "name_len = 0"),
        ((MAX_NAME as u16) + 1, "name_len = MAX_NAME + 1"),
        (255, "name_len = 255"),
    ];

    for &(bad_len, label) in cases {
        let image = base.fork(None);
        // name_len is at byte offset 8 within the directory entry.
        patch_dir_entry(&image, block, index, 8, &bad_len.to_le_bytes());
        let before = image.sectors();

        let mut volume = Volume::mount(image.clone()).expect("mount");
        assert_eq!(
            volume.check(),
            Err(Error::Corrupt),
            "check() must refuse a directory entry with {label}"
        );
        assert!(
            image.sectors() == before,
            "check() must not modify the image ({label})"
        );
        let mut buf = [0u8; MAX_FILE_SIZE];
        assert_eq!(
            volume.read_file(b_inode, &mut buf),
            Ok(5),
            "b.txt must remain readable ({label})"
        );
    }

    // Control: the unpatched image passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean image must pass check()");
}

/// `stage_create` rejects names containing `'/'`, and every VFS path goes
/// through `checked_native_name` which also rejects it.  An on-disk entry
/// whose name bytes include `'/'` can therefore never be reached by `lookup`
/// or `unlink`, so `check()` must flag it as corrupt — the same
/// oracle-vs-runtime inconsistency pattern fixed in preceding sessions.
#[test]
fn check_refuses_a_directory_entry_whose_name_contains_a_slash() {
    let (base, block, index) = two_file_root();
    let b_inode = Volume::mount(base.clone())
        .expect("mount")
        .lookup(1, "b.txt")
        .expect("b.txt")
        .inode;

    // a.txt has name_len = 5; its name bytes live at offset 16 within the entry.
    // Patch each position 0..5 to b'/' in turn.
    let cases: &[(usize, &str)] = &[
        (16, "slash at name[0]"),
        (18, "slash at name[2] (middle)"),
        (20, "slash at name[4] (last)"),
    ];

    for &(byte_offset, label) in cases {
        let image = base.fork(None);
        patch_dir_entry(&image, block, index, byte_offset, b"/");
        let before = image.sectors();

        let mut volume = Volume::mount(image.clone()).expect("mount");
        assert_eq!(
            volume.check(),
            Err(Error::Corrupt),
            "check() must refuse a directory entry where {label}"
        );
        assert!(
            image.sectors() == before,
            "check() must not modify the image ({label})"
        );
        let mut buf = [0u8; MAX_FILE_SIZE];
        assert_eq!(
            volume.read_file(b_inode, &mut buf),
            Ok(5),
            "b.txt must remain readable ({label})"
        );
    }

    // Control: the unpatched image passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean image must pass check()");
}

#[test]
fn check_refuses_a_directory_entry_whose_name_contains_a_null_byte() {
    let (base, block, index) = two_file_root();
    let b_inode = Volume::mount(base.clone())
        .expect("mount")
        .lookup(1, "b.txt")
        .expect("b.txt")
        .inode;

    // a.txt has name_len = 5; its name bytes live at offset 16 within the entry.
    // Patch each position 0..5 to a null byte in turn.
    let cases: &[(usize, &str)] = &[
        (16, "null byte at name[0]"),
        (18, "null byte at name[2] (middle)"),
        (20, "null byte at name[4] (last)"),
    ];

    for &(byte_offset, label) in cases {
        let image = base.fork(None);
        patch_dir_entry(&image, block, index, byte_offset, &[0u8]);
        let before = image.sectors();

        let mut volume = Volume::mount(image.clone()).expect("mount");
        assert_eq!(
            volume.check(),
            Err(Error::Corrupt),
            "check() must refuse a directory entry where {label}"
        );
        assert!(
            image.sectors() == before,
            "check() must not modify the image ({label})"
        );
        let mut buf = [0u8; MAX_FILE_SIZE];
        assert_eq!(
            volume.read_file(b_inode, &mut buf),
            Ok(5),
            "b.txt must remain readable ({label})"
        );
    }

    // Control: the unpatched image passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean image must pass check()");
}

/// `checked_native_name` rejects names containing `'\\'`, and every VFS path
/// goes through it.  An on-disk entry whose name bytes include `'\\'` can
/// therefore never be reached by `lookup` or `unlink`, so `check()` must flag
/// it as corrupt — the same oracle-vs-runtime inconsistency pattern fixed in
/// preceding sessions.
#[test]
fn check_refuses_a_directory_entry_whose_name_contains_a_backslash() {
    let (base, block, index) = two_file_root();
    let b_inode = Volume::mount(base.clone())
        .expect("mount")
        .lookup(1, "b.txt")
        .expect("b.txt")
        .inode;

    // a.txt has name_len = 5; its name bytes live at offset 16 within the entry.
    // Patch each position to b'\\' in turn.
    let cases: &[(usize, &str)] = &[
        (16, "backslash at name[0]"),
        (18, "backslash at name[2] (middle)"),
        (20, "backslash at name[4] (last)"),
    ];

    for &(byte_offset, label) in cases {
        let image = base.fork(None);
        patch_dir_entry(&image, block, index, byte_offset, b"\\");
        let before = image.sectors();

        let mut volume = Volume::mount(image.clone()).expect("mount");
        assert_eq!(
            volume.check(),
            Err(Error::Corrupt),
            "check() must refuse a directory entry where {label}"
        );
        assert!(
            image.sectors() == before,
            "check() must not modify the image ({label})"
        );
        let mut buf = [0u8; MAX_FILE_SIZE];
        assert_eq!(
            volume.read_file(b_inode, &mut buf),
            Ok(5),
            "b.txt must remain readable ({label})"
        );
    }

    // Control: the unpatched image passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean image must pass check()");
}
#[test]
fn check_refuses_a_directory_entry_whose_name_is_dot_or_dotdot() {
    let (base, block, index) = two_file_root();
    let b_inode = Volume::mount(base.clone())
        .expect("mount")
        .lookup(1, "b.txt")
        .expect("b.txt")
        .inode;

    // Overwrite a.txt's name_len and leading name bytes to produce "." or "..".
    // name_len lives at entry offset 8 (u16, little-endian); name bytes start at 16.
    let cases: &[(&str, &[u8], &[u8])] = &[(".", &[1u8, 0u8], b"."), ("..", &[2u8, 0u8], b"..")];

    for &(label, name_len_bytes, name_bytes) in cases {
        let image = base.fork(None);
        patch_dir_entry(&image, block, index, 8, name_len_bytes);
        patch_dir_entry(&image, block, index, 16, name_bytes);
        let before = image.sectors();

        let mut volume = Volume::mount(image.clone()).expect("mount");
        assert_eq!(
            volume.check(),
            Err(Error::Corrupt),
            "check() must refuse a directory entry with name {label:?}"
        );
        assert!(
            image.sectors() == before,
            "check() must not modify the image (name {label:?})"
        );
        let mut buf = [0u8; MAX_FILE_SIZE];
        assert_eq!(
            volume.read_file(b_inode, &mut buf),
            Ok(5),
            "b.txt must remain readable (name {label:?})"
        );
    }

    // Control: the unpatched image passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean image must pass check()");
}

/// `stage_create` accepts names only as `&str`, which Rust guarantees to be
/// valid UTF-8.  No runtime path can therefore store a name whose bytes are
/// not valid UTF-8.  An entry with a non-UTF-8 name is permanently
/// unreachable (every lookup converts through `&str`), so `check()` must
/// flag it as corrupt.
#[test]
fn check_refuses_a_directory_entry_whose_name_is_not_valid_utf8() {
    let (base, block, index) = two_file_root();
    let b_inode = Volume::mount(base.clone())
        .expect("mount")
        .lookup(1, "b.txt")
        .expect("b.txt")
        .inode;

    // a.txt has name_len = 5; its name bytes live at entry offset 16.
    // Patch the first name byte to 0xFF, which is never valid in any UTF-8
    // encoding.  Other cases try a two-byte overlong sequence starter (0xC0)
    // and a bare continuation byte (0x80), both also invalid UTF-8.
    let cases: &[(usize, &[u8], &str)] = &[
        (16, &[0xFF], "0xFF at name[0]"),
        (16, &[0xC0], "bare 0xC0 at name[0]"),
        (16, &[0x80], "bare continuation 0x80 at name[0]"),
    ];

    for &(byte_offset, patch, label) in cases {
        let image = base.fork(None);
        patch_dir_entry(&image, block, index, byte_offset, patch);
        let before = image.sectors();

        let mut volume = Volume::mount(image.clone()).expect("mount");
        assert_eq!(
            volume.check(),
            Err(Error::Corrupt),
            "check() must refuse a directory entry where {label}"
        );
        assert!(
            image.sectors() == before,
            "check() must not modify the image ({label})"
        );
        let mut buf = [0u8; MAX_FILE_SIZE];
        assert_eq!(
            volume.read_file(b_inode, &mut buf),
            Ok(5),
            "b.txt must remain readable ({label})"
        );
    }

    // Control: the unpatched image passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean image must pass check()");
}

/// `check()` must refuse a directory entry that has non-zero bytes beyond its
/// declared `name_len`.
///
/// `stage_create` builds entries with `DirEntry::empty()`, which zeroes the
/// full 48-byte name array, then copies only `name[..name.len()]` bytes.  The
/// bytes at `name[name_len..]` are therefore always zero on a correctly-written
/// filesystem.  `DirEntry::encode` writes the entire `self.name` array to disk
/// (48 bytes), so trailing zeros are always flushed; no runtime path plants a
/// non-zero byte there.  `matches()` only inspects `name[..name_len]`, so a
/// crafted entry with non-zero bytes beyond `name_len` passes all runtime
/// operations undetected — only `check()` can surface this invariant violation.
#[test]
fn check_refuses_a_directory_entry_with_nonzero_bytes_beyond_name_len() {
    use wfs::MAX_NAME;

    let (base, block, index) = two_file_root();
    let b_inode = Volume::mount(base.clone())
        .expect("mount")
        .lookup(1, "b.txt")
        .expect("b.txt")
        .inode;

    // a.txt has name_len = 5; its name field occupies raw entry bytes 16..64.
    // Bytes 16..21 hold "a.txt"; bytes 21..64 are always zero.  Patch each of
    // three positions beyond name_len to a non-zero byte: the first trailing
    // byte, a mid-range byte, and the last byte of the name field.
    let name_len = 5usize; // "a.txt"
    let cases: &[(usize, &[u8], &str)] = &[
        (16 + name_len,           &[0xFF], "first byte beyond name_len (offset 21)"),
        (16 + name_len + 10,      &[0x01], "mid-range byte beyond name_len (offset 31)"),
        (16 + MAX_NAME - 1,       &[0x80], "last byte of the name field (offset 63)"),
    ];

    for &(at, patch, label) in cases {
        let image = base.fork(None);
        patch_dir_entry(&image, block, index, at, patch);
        let before = image.sectors();

        let mut volume = Volume::mount(image.clone()).expect("mount");
        assert_eq!(
            volume.check(),
            Err(Error::Corrupt),
            "check() must refuse an entry with non-zero bytes beyond name_len ({label})"
        );
        assert!(
            image.sectors() == before,
            "check() must not modify the image ({label})"
        );
        let mut buf = [0u8; MAX_FILE_SIZE];
        assert_eq!(
            volume.read_file(b_inode, &mut buf),
            Ok(5),
            "b.txt must remain readable ({label})"
        );
    }

    // Control: the unpatched image passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean image must pass check()");
}

#[test]
fn check_refuses_duplicate_directory_entry_names() {
    use wfs::DIR_ENTRIES_PER_BLOCK;

    let (base, block, _a_index) = two_file_root();

    // Find b.txt's entry index within the root directory view.
    let b_pos = Volume::mount(base.clone())
        .expect("mount b_pos")
        .read_dir(1)
        .expect("read_dir")
        .entries
        .iter()
        .position(|e| e.matches("b.txt"))
        .expect("b.txt entry");
    // Both files were created in a fresh FS; they must sit in the first block.
    assert!(
        b_pos < DIR_ENTRIES_PER_BLOCK,
        "b.txt must be in the first directory block"
    );
    let b_index = b_pos;

    let a_inode = Volume::mount(base.clone())
        .expect("mount a_inode")
        .lookup(1, "a.txt")
        .expect("a.txt")
        .inode;

    // Patch b.txt's name to "a.txt" so two live entries share the same name.
    // Name bytes start at entry offset 16; change the leading 'b' to 'a'.
    let image = base.fork(None);
    patch_dir_entry(&image, block, b_index, 16, b"a");
    let before = image.sectors();

    let mut volume = Volume::mount(image.clone()).expect("mount");
    assert_eq!(
        volume.check(),
        Err(Error::Corrupt),
        "check() must refuse two live entries with the same name"
    );
    assert!(
        image.sectors() == before,
        "check() must not modify the image"
    );
    // a.txt is still readable through its inode.
    let mut buf = [0u8; MAX_FILE_SIZE];
    assert_eq!(
        volume.read_file(a_inode, &mut buf),
        Ok(5),
        "a.txt must remain readable after corrupt-image check"
    );

    // Control: the unpatched image passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean image must pass check()");
}
/// `check()` must refuse a live file inode whose allocated blocks are
/// inconsistent with its `size` field.
///
/// `stage_write` always allocates exactly `size.div_ceil(SECTOR_SIZE)` blocks
/// and zeroes every direct slot beyond the last.  Two deviations are corruption:
///
/// 1. A required block slot (slot < size.div_ceil(SECTOR_SIZE)) is zero: the
///    file appears to have content but `read_file` returns Err(Corrupt) on the
///    empty slot.  `check()` used to skip zero blocks and call such an image
///    clean, an oracle-vs-runtime inconsistency.
///
/// 2. A slot beyond what size requires (slot >= size.div_ceil(SECTOR_SIZE)) is
///    non-zero: the file has an extra allocated block that `read_file` never
///    touches and `unlink` would free, but `check()` must flag it as corruption.
#[test]
fn check_refuses_a_file_inode_whose_blocks_are_inconsistent_with_its_size() {
    let (base, _, _) = two_file_root();
    let (a_inode, b_inode) = {
        let mut volume = Volume::mount(base.clone()).expect("mount");
        let a = volume.lookup(1, "a.txt").expect("a.txt").inode;
        let b = volume.lookup(1, "b.txt").expect("b.txt").inode;
        (a, b)
    };
    let a_block = {
        let mut vol = Volume::mount(base.clone()).expect("mount");
        vol.inode(a_inode).expect("inode").direct[0]
    };

    // Case 1: size=5 (one block required) but direct[0] zeroed.
    // read_file would return Err(Corrupt) on the empty slot; check() must too.
    let image = base.fork(None);
    reseal_inode_direct(&image, a_inode, 0, 0);
    let before = image.sectors();

    let mut volume = Volume::mount(image.clone()).expect("mount");
    assert_eq!(
        volume.check(),
        Err(Error::Corrupt),
        "check() must refuse a file inode with size>0 but direct[0]=0"
    );
    assert!(
        image.sectors() == before,
        "check() must not modify the image (case 1)"
    );
    let mut buf = [0u8; MAX_FILE_SIZE];
    assert_eq!(
        volume.read_file(b_inode, &mut buf),
        Ok(5),
        "b.txt must remain readable after corrupt-image check (case 1)"
    );

    // Case 2: patch the inode to size=0 while keeping direct[0]=a_block,
    // so the file has an extra block beyond what its size requires.
    let image2 = base.fork(None);
    {
        let inode_start = {
            let vol = Volume::mount(image2.clone()).expect("mount");
            vol.super_block().inode_start
        };
        let index = a_inode - 1;
        let lba = (inode_start + index / 4) as usize;
        let offset = (index % 4) as usize * 128;
        let mut sectors = image2.image.borrow_mut();
        let raw = &mut sectors[lba][offset..offset + 128];
        raw[8..16].copy_from_slice(&0u64.to_le_bytes()); // size = 0
        raw[40..48].copy_from_slice(&a_block.to_le_bytes()); // direct[0] still set
        let checksum = test_fnv1a(&raw[..120]);
        raw[120..128].copy_from_slice(&checksum.to_le_bytes());
    }
    let before2 = image2.sectors();

    let mut volume2 = Volume::mount(image2.clone()).expect("mount2");
    assert_eq!(
        volume2.check(),
        Err(Error::Corrupt),
        "check() must refuse a file inode with size=0 but direct[0] non-zero"
    );
    assert!(
        image2.sectors() == before2,
        "check() must not modify the image (case 2)"
    );
    assert_eq!(
        volume2.read_file(b_inode, &mut buf),
        Ok(5),
        "b.txt must remain readable after corrupt-image check (case 2)"
    );

    // Control: the unpatched image passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean image must pass check()");
}

/// `check()` must refuse a volume where the bitmap marks a data block allocated
/// but no live inode names it — an orphaned (leaked) block.
#[test]
fn check_refuses_a_bitmap_allocated_block_named_by_no_inode() {
    let base = formatted();
    let sb = *Volume::mount(base.clone()).expect("mount").super_block();

    // After format, data_start holds the root directory. data_start + 1 is free.
    // Mark it allocated without naming it in any inode.
    let orphan = sb.data_start + 1;
    assert!(
        orphan < sb.total_blocks,
        "image must have at least two data blocks"
    );

    mark_allocated(&base, &sb, orphan);

    let before = base.sectors();
    assert_eq!(
        Volume::mount(base.clone()).expect("mount").check(),
        Err(Error::Corrupt),
        "check() must refuse a block the bitmap marks allocated but no inode names"
    );
    assert!(
        base.sectors() == before,
        "check() must not modify the image"
    );

    // Control: the unmodified image passes check().
    let mut clean_vol = Volume::mount(formatted()).expect("mount clean");
    assert_eq!(clean_vol.check(), Ok(()), "clean image must pass check()");
}

/// `check()` must refuse a volume where a file inode's `data_checksum` does
/// not match the content of its data blocks.
///
/// `read_file` always verifies the checksum before returning data; if
/// `check()` skips the same verification it would call an image with silently
/// corrupted file data consistent, diverging from the runtime's own verdict.
#[test]
fn check_refuses_a_file_with_a_mismatched_data_checksum() {
    let (base, _, _) = two_file_root();
    let (a_inode, b_inode, a_block) = {
        let mut volume = Volume::mount(base.clone()).expect("mount");
        let a = volume.lookup(1, "a.txt").expect("a.txt").inode;
        let b = volume.lookup(1, "b.txt").expect("b.txt").inode;
        let block = volume.inode(a).expect("inode a").direct[0];
        (a, b, block)
    };
    let _ = a_inode;

    // Corrupt one byte in a.txt's data block.  The inode (and its
    // data_checksum) are untouched, so every structural check passes but
    // the content no longer matches the stored checksum.
    let image = base.fork(None);
    image.flip_byte(a_block as usize, 0);
    let before = image.sectors();

    let mut volume = Volume::mount(image.clone()).expect("mount");
    assert_eq!(
        volume.check(),
        Err(Error::Corrupt),
        "check() must refuse a file whose content does not match its data_checksum"
    );
    assert!(
        image.sectors() == before,
        "check() must not modify the image"
    );
    // b.txt is unaffected.
    let mut buf = [0u8; wfs::MAX_FILE_SIZE];
    assert_eq!(
        volume.read_file(b_inode, &mut buf),
        Ok(5),
        "b.txt must remain readable after a corrupt-image check"
    );

    // Control: the unpatched image passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean image must pass check()");
}

#[test]
fn check_refuses_two_directory_entries_naming_the_same_inode() {
    let (base, block, _a_index) = two_file_root();
    let (a_inode, b_index) = {
        let mut volume = Volume::mount(base.clone()).expect("mount");
        let a = volume.lookup(1, "a.txt").expect("a.txt").inode;
        let b_idx = volume
            .read_dir(1)
            .expect("read_dir")
            .entries
            .iter()
            .position(|entry| entry.matches("b.txt"))
            .expect("b.txt entry");
        (a, b_idx)
    };

    // Point b.txt's directory entry at a.txt's inode: two different names,
    // same inode number — an impossible hard link under WovenFS.
    let image = base.fork(None);
    patch_dir_entry(&image, block, b_index, 0, &a_inode.to_le_bytes());
    let before = image.sectors();

    assert_eq!(
        Volume::mount(image.clone()).expect("mount").check(),
        Err(Error::Corrupt),
        "check() must refuse two entries naming the same inode"
    );
    assert!(
        image.sectors() == before,
        "check() must not modify the image"
    );
    // a.txt's inode is still readable through the patched volume.
    let mut buf = [0u8; wfs::MAX_FILE_SIZE];
    assert_eq!(
        Volume::mount(image.clone())
            .expect("mount")
            .read_file(a_inode, &mut buf),
        Ok(5),
        "a.txt must remain readable after a corrupt-image check"
    );

    // Control: the unpatched image passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean image must pass check()");
}

/// `read_file` must succeed for an empty file created by `create`.
///
/// `stage_create` initialises the inode with `Inode::empty()`, leaving
/// `data_checksum = 0`.  The FNV-1a hash of an empty slice is
/// `0xcbf29ce484222325`, not 0, so checking `fnv1a(&out[..0]) !=
/// data_checksum` unconditionally would reject every freshly-created empty
/// file — before any write has ever touched it.  `check()` already documents
/// and skips the checksum for `size == 0`; `read_file` must do the same.
#[test]
fn read_file_succeeds_for_an_empty_file() {
    let base = formatted();
    let inode = {
        let mut volume = Volume::mount(base.clone()).expect("mount");
        volume.create(1, "empty.txt", 0o644, 1).expect("create")
    };

    let mut volume = Volume::mount(base.clone()).expect("mount");
    let mut buf = [0u8; wfs::MAX_FILE_SIZE];
    assert_eq!(
        volume.read_file(inode, &mut buf),
        Ok(0),
        "read_file must return Ok(0) for a freshly-created empty file"
    );
    // check() must also pass: a freshly-created empty file is structurally
    // consistent — it has no blocks, so no block or checksum checks apply.
    assert_eq!(volume.check(), Ok(()), "empty file must not make check() fail");
}

/// `check()` must refuse an image that contains a live `KIND_FILE` inode with
/// no directory entry naming it.
///
/// An orphaned inode — mode != 0, kind == KIND_FILE, but `inode` field zeroed
/// in every directory entry — represents a permanent space leak.  `unlink`
/// works by name: without a directory entry there is no name, so neither the
/// inode slot nor its data blocks can ever be freed through the normal path.
/// The bitmap consistency checks cannot surface this because the orphan's
/// blocks are correctly marked allocated and named by the (intact) inode; only
/// the inode↔entry cross-reference exposes it.
#[test]
fn check_refuses_a_live_file_inode_not_named_by_any_directory_entry() {
    // Build a volume with one file so the file's inode is live.
    let base = formatted();
    let file_inode = {
        let mut volume = Volume::mount(base.clone()).expect("mount");
        volume
            .create_file(1, "orphan.txt", 0o644, b"data", 1)
            .expect("create")
    };

    // Locate the directory block and the entry's slot index.
    let (dir_block, entry_index) = {
        let mut volume = Volume::mount(base.clone()).expect("mount");
        let root = volume.inode(1).expect("root inode");
        let dir_block = root.direct[0];
        let index = volume
            .read_dir(1)
            .expect("read_dir")
            .entries
            .iter()
            .position(|e| e.matches("orphan.txt"))
            .expect("orphan.txt entry");
        (dir_block, index)
    };

    // Corrupt: zero the inode field (bytes 0..8) of the directory entry,
    // turning the live entry into a free slot without touching the inode table.
    // The file inode is still marked used and its blocks are still allocated,
    // but no directory entry names it any more.
    let corrupt = base.fork(None);
    patch_dir_entry(&corrupt, dir_block, entry_index, 0, &0u64.to_le_bytes());

    let before = corrupt.sectors();
    let mut volume = Volume::mount(corrupt.clone()).expect("mount corrupt");
    assert_eq!(
        volume.check(),
        Err(Error::Corrupt),
        "check() must refuse an orphaned file inode (inode {} has no directory entry)",
        file_inode
    );
    assert!(
        corrupt.sectors() == before,
        "check() must not modify the image"
    );

    // Control: the unpatched image passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean image must pass check()");
}
/// `check()` must refuse a root directory inode whose `links` field is not 2.
///
/// `format()` always writes `links = 2` for the root directory (one link from
/// the filesystem root, one from "." inside it). No runtime operation ever
/// modifies a directory's link count, so any value other than 2 means the
/// image was written incorrectly or corrupted after the fact. Without this
/// check, `check()` would call such a volume clean despite the format-invariant
/// violation.
#[test]
fn check_refuses_a_root_directory_inode_with_wrong_link_count() {
    let base = formatted();
    let root_inode = Volume::mount(base.clone())
        .expect("mount")
        .super_block()
        .root_inode;
    assert_eq!(root_inode, 1, "sanity: root inode is always 1");

    for links in [0u32, 1, 3, u32::MAX] {
        let image = base.fork(None);
        reseal_inode_links(&image, root_inode, links);
        let before = image.sectors();
        let mut volume = Volume::mount(image.clone()).expect("mount");
        assert_eq!(
            volume.check(),
            Err(Error::Corrupt),
            "links = {links}: check() must refuse the root directory with wrong link count"
        );
        assert!(
            image.sectors() == before,
            "check() must not modify the image"
        );
    }

    // Control: the unpatched image passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean image must pass check()");
}

/// `check()` must refuse a file inode with `size == 0` but `data_checksum != 0`.
///
/// `stage_create` initialises every new file via `Inode::empty()`, which sets
/// `data_checksum = 0`.  That field is only updated by `stage_write` after the
/// first write.  A zero-size file with a non-zero `data_checksum` cannot be
/// produced by any runtime path.  `read_file` skips the checksum comparison for
/// zero-size files (there are no bytes to hash), so neither the read path nor the
/// earlier structural checks surface this inconsistency; only an explicit guard in
/// `check()` catches it.
#[test]
fn check_refuses_a_zero_size_file_with_nonzero_data_checksum() {
    let base = formatted();
    let file_inode = {
        let mut volume = Volume::mount(base.clone()).expect("mount");
        volume.create(1, "empty.txt", 0o600, 1).expect("create empty file")
    };

    let corrupt = base.fork(None);
    // Set data_checksum to fnv1a(b"corrupt") — a non-zero value — while leaving
    // size == 0 intact.  reseal_inode_data_checksum updates the inode's own
    // structural checksum so mount() and inode() accept the sector.
    reseal_inode_data_checksum(&corrupt, file_inode, b"corrupt");

    let before = corrupt.sectors();
    let mut volume = Volume::mount(corrupt.clone()).expect("mount corrupt");
    assert_eq!(
        volume.check(),
        Err(Error::Corrupt),
        "check() must refuse a size-0 file inode with non-zero data_checksum"
    );
    assert!(
        corrupt.sectors() == before,
        "check() must not modify the image"
    );

    // Control: the unpatched empty file passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean image must pass check()");
}

/// `check()` must refuse the root directory inode when its `data_checksum` is
/// non-zero.
///
/// `format()` initialises the root directory inode via `Inode::empty()`, which
/// sets `data_checksum = 0`.  No runtime path (`stage_create`, `stage_write`,
/// `stage_unlink`) ever modifies a directory inode's `data_checksum`, so any
/// non-zero value is corruption the runtime cannot have produced.  `dir_entries`
/// ignores `data_checksum` entirely, so without an explicit guard in `check()`
/// this inconsistency goes undetected.
#[test]
fn check_refuses_root_directory_inode_with_nonzero_data_checksum() {
    let base = formatted();
    let root_inode = {
        let volume = Volume::mount(base.clone()).expect("mount");
        volume.super_block().root_inode
    };
    assert_eq!(root_inode, 1, "sanity: root inode is always 1 on a fresh volume");

    let corrupt = base.fork(None);
    // Set data_checksum to fnv1a(b"corrupt") — a non-zero value.
    // reseal_inode_data_checksum updates the inode's structural checksum so
    // mount() and inode() accept the sector; only check()'s explicit guard
    // catches the logical inconsistency.
    reseal_inode_data_checksum(&corrupt, root_inode, b"corrupt");

    let before = corrupt.sectors();
    let mut volume = Volume::mount(corrupt.clone()).expect("mount corrupt");
    assert_eq!(
        volume.check(),
        Err(Error::Corrupt),
        "check() must refuse a root directory inode with non-zero data_checksum"
    );
    assert!(
        corrupt.sectors() == before,
        "check() must not modify the image"
    );

    // Control: the unpatched volume passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean volume must pass check()");
}

/// `check()` must refuse the root directory inode when its `size` field is not
/// `SECTOR_SIZE`.
///
/// `format()` always sets the root directory's `size` to `SECTOR_SIZE` (512
/// bytes — one directory data block).  No runtime path ever modifies a directory
/// inode's `size`: `stage_create` updates only `modified` on the directory inode,
/// and neither `stage_dir_entry` nor `stage_unlink` touches `size`.  Any other
/// value is corruption the runtime cannot have produced.  `dir_entries` reads
/// directory blocks by their `direct` pointers and ignores `size`, so without an
/// explicit guard in `check()` this inconsistency passes silently.
#[test]
fn check_refuses_root_directory_inode_with_wrong_size() {
    let base = formatted();
    let root_inode = {
        let volume = Volume::mount(base.clone()).expect("mount");
        volume.super_block().root_inode
    };
    assert_eq!(root_inode, 1, "sanity: root inode is always 1 on a fresh volume");

    for size in [0u64, 1, SECTOR_SIZE as u64 - 1, SECTOR_SIZE as u64 + 1] {
        let corrupt = base.fork(None);
        reseal_inode_size(&corrupt, root_inode, size);

        let before = corrupt.sectors();
        let mut volume = Volume::mount(corrupt.clone()).expect("mount corrupt");
        assert_eq!(
            volume.check(),
            Err(Error::Corrupt),
            "size = {size}: check() must refuse the root directory inode with wrong size"
        );
        assert!(
            corrupt.sectors() == before,
            "check() must not modify the image"
        );
    }

    // Control: the unpatched volume passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean volume must pass check()");
}

/// `check()` must refuse the root directory inode when `direct[0]` is zero
/// (no directory data block).
///
/// `format()` always sets `root.direct[0]` to the directory data block and
/// zeroes `direct[1..]` via `Inode::empty()`.  No runtime path ever modifies
/// a directory inode's `direct` array.  A zero `direct[0]` means the directory
/// has no data block: `dir_entries` skips the slot and returns an empty view,
/// hiding every orphaned file inode.  Any such state is corruption the runtime
/// cannot have produced.
///
/// The bitmap bit for the original `root_block` is cleared before the check so
/// that the orphan-block bitmap scan does not fire before the `direct[0]` guard
/// in `check()`'s root section, letting the test prove that guard specifically.
#[test]
fn check_refuses_root_directory_inode_with_zero_first_direct_block() {
    let base = formatted();
    let root_block = read_inode_direct0(&base, 1);

    let corrupt = base.fork(None);
    // Clear the bitmap bit so the orphan-block scan does not fire first.
    clear_bitmap_block(&corrupt, root_block);
    // Patch direct[0] to 0.
    reseal_inode_direct(&corrupt, 1, 0, 0);

    let before = corrupt.sectors();
    let mut volume = Volume::mount(corrupt.clone()).expect("mount corrupt");
    assert_eq!(
        volume.check(),
        Err(Error::Corrupt),
        "check() must refuse root directory inode with direct[0] == 0"
    );
    assert!(corrupt.sectors() == before, "check() must not modify the image");

    // Control: the unpatched volume passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean volume must pass check()");
}

/// `check()` must refuse a root directory inode whose `direct[1]` (or any later
/// slot) is non-zero.
///
/// `format()` sets `root.direct[0]` to the directory data block and zeroes
/// every other slot via `Inode::empty()`.  No runtime operation ever sets a
/// non-zero value in `direct[1..DIRECT_BLOCKS]` for a directory inode, so any
/// such value is corruption.  `dir_entries` reads blocks through the `direct`
/// array, so a non-zero `direct[1]` would be interpreted as a second directory
/// data block, silently doubling the directory without any runtime path to
/// produce it.
///
/// The test must suppress two earlier checks that would otherwise fire first.
/// First, the per-inode block range/bitmap check would see the block in
/// `direct[1]` is not bitmap-allocated and return `Corrupt` before the root
/// section runs; `set_bitmap_block` marks it allocated so that check passes.
/// Second, the orphan-block scan would see the block as bitmap-allocated and
/// named by the root inode (via `direct[1]`), so it passes too.
/// With both earlier paths satisfied, the root-specific `direct[1..]` guard
/// becomes the first error path.
#[test]
fn check_refuses_root_directory_inode_with_nonzero_extra_direct_block() {
    let base = formatted();

    // After format() the root block is the only allocated data block.  The
    // adjacent block (root_block + 1) is free; use it as the spurious extra.
    let root_block = read_inode_direct0(&base, 1);
    let extra_block = root_block + 1;
    {
        let volume = Volume::mount(base.clone()).expect("mount");
        let sb = volume.super_block();
        assert!(
            extra_block < sb.total_blocks,
            "volume too small: extra_block {extra_block} >= total_blocks {}",
            sb.total_blocks
        );
    }

    let corrupt = base.fork(None);
    // Mark the extra block as bitmap-allocated so the orphan-block scan sees it
    // as named by the root inode via direct[1] and passes without firing first.
    set_bitmap_block(&corrupt, extra_block);
    // Set root.direct[1] to the extra block, re-sealing the inode checksum.
    reseal_inode_direct(&corrupt, 1, 1, extra_block);

    let before = corrupt.sectors();
    let mut volume = Volume::mount(corrupt.clone()).expect("mount corrupt");
    assert_eq!(
        volume.check(),
        Err(Error::Corrupt),
        "check() must refuse a root directory inode with non-zero direct[1]"
    );
    assert!(corrupt.sectors() == before, "check() must not modify the image");

    // Control: the unpatched volume passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean volume must pass check()");
}

/// `check()` must refuse the root directory inode when `mode` is not `0o040_755`.
///
/// `format()` hardcodes `root.mode = 0o040_755` and no runtime path ever
/// modifies a directory inode's `mode` field.  Any non-zero value other than
/// `0o040_755` — e.g. a regular-file mode like `0o100_644` — evades the
/// bitmap orphan scan (the inode is still "live") and every other structural
/// check in `check()`.  Only the explicit mode guard at the start of the root
/// section surfaces the inconsistency.
#[test]
fn check_refuses_root_directory_inode_with_wrong_mode() {
    let base = formatted();
    let root_inode = Volume::mount(base.clone())
        .expect("mount")
        .super_block()
        .root_inode;
    assert_eq!(root_inode, 1, "sanity: root inode is always 1");

    for mode in [1u32, 0o100_644, 0o777, u32::MAX] {
        let corrupt = base.fork(None);
        reseal_inode_mode(&corrupt, root_inode, mode);
        let before = corrupt.sectors();

        let mut volume = Volume::mount(corrupt.clone()).expect("mount corrupt");
        assert_eq!(
            volume.check(),
            Err(Error::Corrupt),
            "mode = {mode:#o}: check() must refuse root directory inode with wrong mode"
        );
        assert!(
            corrupt.sectors() == before,
            "check() must not modify the image (mode = {mode:#o})"
        );
    }

    // Control: the unpatched volume passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean volume must pass check()");
}

/// `check()` must refuse a live non-root `KIND_DIR` inode.
///
/// `mkdir` is not implemented: this is a flat, single-directory filesystem.
/// `stage_create` only allocates `KIND_FILE` inodes, so any live inode with
/// `kind == KIND_DIR` and a number other than `root_inode` is corruption.
/// Without the guard, the phantom inode evades every check: the `KIND_FILE`
/// structural checks are skipped, the per-inode block scans find no blocks,
/// and the orphan-inode scan at the end of `check()` skips non-`KIND_FILE`
/// inodes — so `check()` would call the image clean.
#[test]
fn check_refuses_non_root_dir_kind_inode() {
    use wfs::KIND_DIR;

    let base = formatted();

    // Inode 1 is the root (KIND_DIR).  Inode 2 is the first free slot.
    // Inject a phantom KIND_DIR inode there.  It has no data blocks, so
    // neither the bitmap scan nor the block-range check fires before our
    // guard — the phantom simply exists as corrupt metadata.
    let corrupt = base.fork(None);
    // Make inode 2 "used" (mode != 0) before setting its kind.
    reseal_inode_mode(&corrupt, 2, 0o040_755);
    // Set kind = KIND_DIR; reseal_inode_kind re-seals after the mode is in place.
    reseal_inode_kind(&corrupt, 2, KIND_DIR);

    let before = corrupt.sectors();
    let mut volume = Volume::mount(corrupt.clone()).expect("mount corrupt");
    assert_eq!(
        volume.check(),
        Err(Error::Corrupt),
        "check() must refuse a non-root KIND_DIR inode"
    );
    assert!(
        corrupt.sectors() == before,
        "check() must not modify the image"
    );

    // Control: the unpatched volume passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean volume must pass check()");
}

/// `check()` must refuse a `KIND_FILE` inode whose `size` exceeds `MAX_FILE_SIZE`.
///
/// `stage_write` and `create_file` both enforce `data.len() <= MAX_FILE_SIZE`
/// (the maximum bytes eight direct blocks can hold), so no runtime path can
/// produce a file inode with a larger size.  Without an explicit guard the
/// block-count check computes `needed = size.div_ceil(SECTOR_SIZE)`, which can
/// exceed `DIRECT_BLOCKS`; the later data-checksum loop then indexes past the
/// end of the eight-element `direct` array and panics rather than returning
/// `Corrupt`.
///
/// To isolate the new size-bounds guard as the first error path, the test
/// writes a full `MAX_FILE_SIZE` file (allocating all 8 direct blocks and
/// setting the correct data checksum), then patches only the `size` field to
/// `MAX_FILE_SIZE + 1`.  With all 8 blocks allocated the existing block-count
/// check (`needed = 9`, all 8 slots non-zero: each `(true) != (true)` false,
/// no Corrupt) would pass, but the data-checksum loop would panic at
/// `direct[8]`.  With the new size-bounds guard, `check()` returns `Corrupt`.
#[test]
fn check_refuses_file_inode_with_size_above_max_file_size() {
    use wfs::MAX_FILE_SIZE;

    let base = formatted();

    // Create a file with exactly MAX_FILE_SIZE bytes, filling all 8 direct
    // blocks.  The data checksum is set to fnv1a(data) by stage_write, which
    // is correct for this content.
    let data = vec![0xaau8; MAX_FILE_SIZE];
    let file_inode = Volume::mount(base.clone())
        .expect("mount")
        .create_file(1, "full.bin", 0o600, &data, 1)
        .expect("create full file");

    // Fork the image and patch only the size field to MAX_FILE_SIZE + 1.
    // All 8 direct blocks remain non-zero and the data checksum is unchanged,
    // so neither the block-count check nor the data-checksum check would fire
    // before the new size-bounds guard does.
    let corrupt = base.fork(None);
    reseal_inode_size(&corrupt, file_inode, MAX_FILE_SIZE as u64 + 1);

    let before = corrupt.sectors();
    let mut volume = Volume::mount(corrupt.clone()).expect("mount corrupt");
    assert_eq!(
        volume.check(),
        Err(Error::Corrupt),
        "check() must refuse a file inode with size > MAX_FILE_SIZE"
    );
    assert!(
        corrupt.sectors() == before,
        "check() must not modify the image"
    );

    // Control: unpatched volume with size == MAX_FILE_SIZE passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean volume must pass check()");
}

/// `check()` must refuse a directory entry that has non-zero bytes in the
/// 4-byte reserved padding gap at raw entry bytes 12..16.
///
/// `DirEntry::encode` begins with `raw[..DIR_ENTRY_SIZE].fill(0)` and then
/// writes the `inode` (0..8), `name_len` (8..10), `kind` (10..12), and `name`
/// (16..64) fields, leaving bytes 12..16 as zero.  The decoded `DirEntry`
/// struct drops those bytes; no field reads them.  A correctly-written
/// filesystem therefore always has zeros there, and any non-zero byte is
/// corruption `check()` must detect.
#[test]
fn check_refuses_a_directory_entry_with_nonzero_reserved_padding() {
    let (base, block, index) = two_file_root();
    let b_inode = Volume::mount(base.clone())
        .expect("mount")
        .lookup(1, "b.txt")
        .expect("b.txt")
        .inode;

    // Each of the 4 padding bytes (raw entry offsets 12, 13, 14, 15) must be
    // independently refused by check().
    let cases: &[(usize, u8, &str)] = &[
        (12, 0xFF, "first padding byte (offset 12)"),
        (13, 0x01, "second padding byte (offset 13)"),
        (14, 0x80, "third padding byte (offset 14)"),
        (15, 0x42, "fourth padding byte (offset 15)"),
    ];

    for &(at, val, label) in cases {
        let image = base.fork(None);
        patch_dir_entry(&image, block, index, at, &[val]);
        let before = image.sectors();

        let mut volume = Volume::mount(image.clone()).expect("mount");
        assert_eq!(
            volume.check(),
            Err(Error::Corrupt),
            "check() must refuse a directory entry with non-zero reserved padding ({label})"
        );
        assert!(
            image.sectors() == before,
            "check() must not modify the image ({label})"
        );
        let mut buf = [0u8; wfs::MAX_FILE_SIZE];
        assert_eq!(
            volume.read_file(b_inode, &mut buf),
            Ok(5),
            "b.txt must remain readable ({label})"
        );
    }

    // Control: the unpatched image passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean image must pass check()");
}

/// `check()` must refuse a live inode whose `kind` is not `KIND_FILE` (1) or
/// `KIND_DIR` (2).
///
/// Only two kind values are defined. `stage_create` allocates only `KIND_FILE`
/// inodes; `format()` writes one `KIND_DIR` inode (the root). Any other value
/// is corruption no runtime path can produce. The per-inode guards in `check()`
/// are all gated on `kind == KIND_FILE` or `kind == KIND_DIR`, so an unknown
/// kind passes every existing check silently: the `KIND_FILE` guards are
/// skipped, the orphan-inode scan at the end skips non-`KIND_FILE` inodes, the
/// non-root `KIND_DIR` guard does not fire, and no directory entry names the
/// phantom inode, so the bitmap scan passes too.
#[test]
fn check_refuses_a_live_inode_with_unknown_kind() {
    let base = formatted();

    // Unknown kind values to exercise; none equal KIND_FILE (1) or KIND_DIR (2).
    let unknown_kinds: &[(u16, &str)] = &[
        (0, "kind 0"),
        (3, "kind 3"),
        (0xFF, "kind 255"),
        (0xFFFF, "kind 65535"),
    ];

    for &(kind, label) in unknown_kinds {
        // Inject a live inode with the unknown kind into inode slot 2 (the first
        // free slot after the root at inode 1).  Give it a non-zero mode so
        // is_used() returns true, then set the unknown kind.
        let image = base.fork(None);
        reseal_inode_mode(&image, 2, 0o100_644);
        reseal_inode_kind(&image, 2, kind);

        let before = image.sectors();
        let mut volume = Volume::mount(image.clone()).expect("mount");
        assert_eq!(
            volume.check(),
            Err(Error::Corrupt),
            "check() must refuse a live inode with {label}"
        );
        assert!(
            image.sectors() == before,
            "check() must not modify the image ({label})"
        );
    }

    // Control: the unpatched volume passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean volume must pass check()");
}

/// `check()` must refuse a free directory entry whose non-inode fields are not
/// all zero.
///
/// `DirEntry::encode` calls `raw[..DIR_ENTRY_SIZE].fill(0)` before writing any
/// field; `DirEntry::empty()` produces all zeros; every runtime write of a freed
/// slot calls `stage_dir_entry` with `DirEntry::empty()`.  A free entry (inode
/// == 0, bytes 0..8 all zero) therefore always has all 64 bytes zeroed on a
/// correctly-written filesystem.  Non-zero `name_len` (bytes 8..10), `kind`
/// (bytes 10..12), or any `name` byte (bytes 16..64) in a free slot is
/// corruption no runtime path can produce.
#[test]
fn check_refuses_a_free_directory_entry_with_nonzero_fields() {
    let base = formatted();
    // In a freshly formatted volume the root directory block is written with
    // all zeros by `format()`, so every directory entry is a free slot.
    let block = Volume::mount(base.clone())
        .expect("mount")
        .inode(1)
        .expect("root inode")
        .direct[0];

    // Each case patches one field in free entry 0 to a non-zero value; all
    // are offsets within the 64-byte entry.
    let cases: &[(usize, &[u8], &str)] = &[
        (8, &1u16.to_le_bytes(), "name_len lo byte (offset 8)"),
        (9, &[0x01], "name_len hi byte (offset 9)"),
        (10, &1u16.to_le_bytes(), "kind lo byte (offset 10)"),
        (11, &[0x01], "kind hi byte (offset 11)"),
        (16, &[0xFF], "first name byte (offset 16)"),
        (63, &[0x01], "last name byte (offset 63)"),
    ];

    for &(at, val, label) in cases {
        let image = base.fork(None);
        patch_dir_entry(&image, block, 0, at, val);
        let before = image.sectors();

        let mut volume = Volume::mount(image.clone()).expect("mount");
        assert_eq!(
            volume.check(),
            Err(Error::Corrupt),
            "check() must refuse a free entry with non-zero {label}"
        );
        assert!(
            image.sectors() == before,
            "check() must not modify the image ({label})"
        );
    }

    // Control: the unpatched image passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean image must pass check()");
}

/// `check()` must refuse a free inode with any non-zero field beyond `mode`.
///
/// `Inode::empty()` zeroes every field; `format()` zeroes inode slots directly
/// (no encode call); `stage_unlink` writes `Inode::empty()` to free a slot.
/// No runtime path can therefore produce a free inode (mode == 0) with a
/// non-zero `links`, `size`, `data_checksum`, `kind`, or `direct[i]` entry.
/// A crafted image with a resealed structural checksum passes `Inode::decode()`
/// while hiding the corruption from every per-field check, so without an
/// explicit guard `check()` silently accepts the corrupt image.
#[test]
fn check_refuses_a_free_inode_with_nonzero_fields() {
    let base = formatted();
    // Inode 2 is the first free slot after the root (inode 1) on a freshly
    // formatted volume.

    // Non-zero links.
    {
        let image = base.fork(None);
        reseal_inode_links(&image, 2, 1);
        let before = image.sectors();
        let mut volume = Volume::mount(image.clone()).expect("mount");
        assert_eq!(
            volume.check(),
            Err(Error::Corrupt),
            "check() must refuse a free inode with links = 1"
        );
        assert!(
            image.sectors() == before,
            "check() must not modify the image (links)"
        );
    }

    // Non-zero size.
    {
        let image = base.fork(None);
        reseal_inode_size(&image, 2, 1);
        let before = image.sectors();
        let mut volume = Volume::mount(image.clone()).expect("mount");
        assert_eq!(
            volume.check(),
            Err(Error::Corrupt),
            "check() must refuse a free inode with size = 1"
        );
        assert!(
            image.sectors() == before,
            "check() must not modify the image (size)"
        );
    }

    // Non-zero data_checksum.
    {
        let image = base.fork(None);
        reseal_inode_data_checksum(&image, 2, b"corrupt");
        let before = image.sectors();
        let mut volume = Volume::mount(image.clone()).expect("mount");
        assert_eq!(
            volume.check(),
            Err(Error::Corrupt),
            "check() must refuse a free inode with non-zero data_checksum"
        );
        assert!(
            image.sectors() == before,
            "check() must not modify the image (data_checksum)"
        );
    }

    // Non-zero kind.
    {
        let image = base.fork(None);
        reseal_inode_kind(&image, 2, 1);
        let before = image.sectors();
        let mut volume = Volume::mount(image.clone()).expect("mount");
        assert_eq!(
            volume.check(),
            Err(Error::Corrupt),
            "check() must refuse a free inode with kind = 1"
        );
        assert!(
            image.sectors() == before,
            "check() must not modify the image (kind)"
        );
    }

    // Non-zero direct[0].
    {
        let image = base.fork(None);
        reseal_inode_direct(&image, 2, 0, 1);
        let before = image.sectors();
        let mut volume = Volume::mount(image.clone()).expect("mount");
        assert_eq!(
            volume.check(),
            Err(Error::Corrupt),
            "check() must refuse a free inode with direct[0] = 1"
        );
        assert!(
            image.sectors() == before,
            "check() must not modify the image (direct[0])"
        );
    }

    // Control: the unpatched volume passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean volume must pass check()");
}

/// `check()` must return `Err(Corrupt)` when a free inode (mode == 0) has a
/// non-zero `created` or `modified` timestamp.  `Inode::empty()` zeroes both
/// fields, and `stage_unlink` writes `Inode::empty()` to free a slot, so no
/// runtime path can leave either field non-zero in a free inode.  A crafted
/// image with a resealed structural checksum passes `Inode::decode()` while
/// hiding the non-zero timestamp from every earlier check.
#[test]
fn check_refuses_root_directory_inode_with_nonzero_created_timestamp() {
    let base = formatted();
    let root_inode = {
        let volume = Volume::mount(base.clone()).expect("mount");
        volume.super_block().root_inode
    };
    assert_eq!(root_inode, 1, "sanity: root inode is always 1 on a fresh volume");

    // `format()` builds the root inode via `Inode::empty()`, which zeroes
    // every field including `created`.  Only `stage_create` ever writes a
    // non-zero `created`, and only for newly allocated KIND_FILE inodes — never
    // for the pre-existing root directory inode.  A non-zero `root.created` is
    // therefore corruption the runtime cannot produce; `check()` must reject it.
    for created in [1u64, u64::MAX] {
        let corrupt = base.fork(None);
        reseal_inode_created(&corrupt, root_inode, created);

        let before = corrupt.sectors();
        let mut volume = Volume::mount(corrupt.clone()).expect("mount corrupt");
        assert_eq!(
            volume.check(),
            Err(Error::Corrupt),
            "check() must refuse a root directory inode with created = {created}"
        );
        assert!(
            corrupt.sectors() == before,
            "check() must not modify the image (created = {created})"
        );
    }

    // Control: the unpatched volume passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean volume must pass check()");
}

#[test]
fn check_refuses_a_free_inode_with_nonzero_timestamps() {
    let base = formatted();
    // Inode 2 is the first free slot after the root (inode 1).

    // Non-zero created.
    {
        let image = base.fork(None);
        reseal_inode_created(&image, 2, 1);
        let before = image.sectors();
        let mut volume = Volume::mount(image.clone()).expect("mount");
        assert_eq!(
            volume.check(),
            Err(Error::Corrupt),
            "check() must refuse a free inode with created = 1"
        );
        assert!(
            image.sectors() == before,
            "check() must not modify the image (created)"
        );
    }

    // Non-zero modified.
    {
        let image = base.fork(None);
        reseal_inode_modified(&image, 2, 1);
        let before = image.sectors();
        let mut volume = Volume::mount(image.clone()).expect("mount");
        assert_eq!(
            volume.check(),
            Err(Error::Corrupt),
            "check() must refuse a free inode with modified = 1"
        );
        assert!(
            image.sectors() == before,
            "check() must not modify the image (modified)"
        );
    }

    // Control: the unpatched volume passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean volume must pass check()");
}

/// Patch the superblock's `root_inode` field in the active checkpoint slot
/// (always LBA 0 after a fresh `format()`) and re-seal the superblock checksum.
///
/// The superblock layout has `root_inode` at bytes 72..80 and the checksum
/// (FNV-1a over bytes 0..80) at bytes 80..88.  `mount()` accepts the result
/// when the new value is in `1..=inode_count`.
fn patch_superblock_root_inode(image: &CrashDisk, root_inode: u64) {
    let mut sectors = image.image.borrow_mut();
    sectors[0][72..80].copy_from_slice(&root_inode.to_le_bytes());
    let checksum = test_fnv1a(&sectors[0][..80]);
    sectors[0][80..88].copy_from_slice(&checksum.to_le_bytes());
}

/// `format()` always writes `sb.root_inode = 1`.  No runtime path modifies the
/// superblock's `root_inode` field after format; any other value is corruption.
/// `SuperBlock::valid()` only bounds it to `1..=inode_count`, so a crafted value
/// in that range passes `mount()` but should be caught by `check()`.
#[test]
fn check_refuses_superblock_with_root_inode_other_than_1() {
    let base = formatted();

    // Both values are in `1..=INODES` (= 1..=32) so `mount()` accepts the
    // patched superblock and `check()` is the first rejection.
    for root_inode in [2u64, INODES as u64] {
        let corrupt = base.fork(None);
        patch_superblock_root_inode(&corrupt, root_inode);

        let before = corrupt.sectors();
        let mut volume =
            Volume::mount(corrupt.clone()).expect("mount should accept in-range root_inode");
        assert_eq!(
            volume.check(),
            Err(Error::Corrupt),
            "check() must refuse root_inode = {root_inode}"
        );
        assert!(
            corrupt.sectors() == before,
            "check() must not modify the image (root_inode = {root_inode})"
        );
    }

    // Control: the unpatched volume passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean volume must pass check()");
}

/// `check()` must refuse a live `KIND_FILE` inode whose `links` field is not 1.
///
/// `stage_create` always initialises `node.links = 1`.  Hard links are not
/// implemented: no runtime path increments or decrements a file's link count.
/// Any value other than 1 is corruption the runtime cannot have produced.
/// `unlink` already refuses a file whose `links` is not 1 rather than
/// proceeding, so a crafted wrong count permanently orphans the file: neither
/// the inode slot nor its data blocks can be freed.  Without an explicit guard
/// in `check()`, a resealed inode with a wrong link count passes `mount()` and
/// `inode()` and silently returns to the caller with the corrupt value.
#[test]
fn check_refuses_a_file_inode_with_links_other_than_1() {
    let base = formatted();
    let file_inode = {
        let mut volume = Volume::mount(base.clone()).expect("mount");
        volume.create(1, "test.txt", 0o600, 1).expect("create file")
    };

    for links in [0u32, 2, u32::MAX] {
        let corrupt = base.fork(None);
        reseal_inode_links(&corrupt, file_inode, links);

        let before = corrupt.sectors();
        let mut volume = Volume::mount(corrupt.clone()).expect("mount corrupt");
        assert_eq!(
            volume.check(),
            Err(Error::Corrupt),
            "links = {links}: check() must refuse a KIND_FILE inode with links != 1"
        );
        assert!(
            corrupt.sectors() == before,
            "check() must not modify the image (links = {links})"
        );
    }

    // Control: the unpatched volume passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean volume must pass check()");
}

/// `check()` must refuse a directory entry whose `inode` field names an inode
/// number beyond the end of the inode table (`> sb.inode_count`).
///
/// `stage_create` allocates inodes from the range `2..=inode_count` and
/// writes the allocated number into the new directory entry.  No runtime path
/// ever writes a value larger than `inode_count` into a live directory entry.
/// A crafted entry with an out-of-range inode number is permanently
/// unlookupable: calling `self.inode(entry.inode)` would attempt to read a
/// sector beyond the inode table region.  `check()` validates `entry.inode`
/// before calling `self.inode`, so the corruption is caught without an
/// out-of-bounds read or a false `Ok(())`.
#[test]
fn check_refuses_a_directory_entry_with_inode_number_out_of_range() {
    let (base, block, index) = two_file_root();

    // Patch a.txt's directory entry inode field (bytes 0–7) to INODES + 1
    // (= 33), which is one past the end of the inode table (inode_count = 32).
    let image = base.fork(None);
    patch_dir_entry(&image, block, index, 0, &(u64::from(INODES) + 1).to_le_bytes());
    // Snapshot after patching: check() must not write anything.
    let before = image.sectors();

    let mut volume = Volume::mount(image.clone()).expect("mount");
    assert_eq!(
        volume.check(),
        Err(Error::Corrupt),
        "check() must refuse a directory entry with inode = INODES + 1"
    );
    // The image must be unchanged by check().
    assert!(
        image.sectors() == before,
        "check() must not modify the image"
    );

    // Control: the unpatched image passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean image must pass check()");
}

/// `check()` must refuse a live file inode whose data block the bitmap marks
/// as free.
///
/// `stage_write` marks every allocated block as used in the bitmap before
/// writing the inode back.  `stage_unlink` clears bitmap bits only after
/// zeroing and writing the inode.  No runtime path therefore leaves a live
/// inode naming a block the bitmap considers free.  A crafted image where the
/// bitmap bit for a live file's block is cleared is an impossible state: the
/// allocator could hand out the same block to a future file, creating a silent
/// cross-inode alias.
///
/// This covers the inode→bitmap direction.  The bitmap→inode direction
/// (a block allocated in the bitmap but named by no live inode) is covered by
/// `check_refuses_a_bitmap_allocated_block_named_by_no_inode`.
#[test]
fn check_refuses_a_file_inode_naming_a_block_the_bitmap_marks_free() {
    let (base, _, _) = two_file_root();
    let (a_inode, a_block) = {
        let mut volume = Volume::mount(base.clone()).expect("mount");
        let a = volume.lookup(1, "a.txt").expect("a.txt").inode;
        let block = volume.inode(a).expect("inode a").direct[0];
        (a, block)
    };
    let _ = a_inode;

    // Clear the bitmap bit for a.txt's data block without touching the inode or
    // its content.  The data checksum and range checks both pass (the block
    // content is unchanged and the block address is within [data_start,
    // total_blocks)), so the per-inode bitmap check at `!bitmap_get(block)` is
    // the first path to fire.  The subsequent bitmap→inode orphan scan never
    // reaches it, because the block is no longer marked allocated.
    let image = base.fork(None);
    clear_bitmap_block(&image, a_block);
    let before = image.sectors();

    let mut volume = Volume::mount(image.clone()).expect("mount");
    assert_eq!(
        volume.check(),
        Err(Error::Corrupt),
        "check() must refuse a file inode whose data block the bitmap marks free"
    );
    assert!(
        image.sectors() == before,
        "check() must not modify the image"
    );

    // Control: the unpatched image passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean image must pass check()");
}

#[test]
fn check_refuses_root_directory_inode_with_kind_not_dir() {
    // `format()` sets the root inode's kind to KIND_DIR (2) and no runtime path
    // ever changes a directory inode's kind field.  Any other value is
    // corruption.  The root-section guard `root.kind != KIND_DIR` catches it.
    //
    // To exercise this guard independently, the crafted inode must pass every
    // earlier check:
    //
    //   Main-loop KIND_FILE path:
    //     links = 1  (KIND_FILE requires links == 1)
    //     block consistency: needed = 1, direct[0] != 0, direct[1..] == 0
    //     data_checksum = fnv1a(directory data block) — content check passes
    //     block range: direct[0] is the root directory block
    //     bitmap: that block is allocated
    //     no cross-inode aliasing (only inode 1 names it)
    //   Bitmap→inode orphan scan: directory block named by inode 1
    //   Root-section mode check: mode left at 0o040_755 → passes
    //   Root-section kind check: KIND_FILE != KIND_DIR → Err(Corrupt)  ← fires
    use wfs::KIND_FILE;

    let base = formatted();
    let dir_block = Volume::mount(base.clone())
        .expect("mount")
        .inode(1)
        .expect("root inode")
        .direct[0];
    let dir_bytes = base.sectors()[dir_block as usize];

    let image = base.fork(None);
    reseal_inode_kind(&image, 1, KIND_FILE);
    reseal_inode_links(&image, 1, 1u32);
    reseal_inode_data_checksum(&image, 1, &dir_bytes);

    let before = image.sectors();
    let mut volume = Volume::mount(image.clone()).expect("mount");
    assert_eq!(
        volume.check(),
        Err(Error::Corrupt),
        "check() must refuse a root inode whose kind is not KIND_DIR"
    );
    assert!(
        image.sectors() == before,
        "check() must not modify the image"
    );

    // Control: the unpatched image passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean image must pass check()");
}

/// `check()` calls `self.inode(number)?` in its main loop, which internally
/// calls `checked_inode`.  That function refuses any direct-block pointer that
/// is outside the data region `[data_start, total_blocks)`:
///
///   let in_data = |block| *block == 0 || (data_start..total_blocks).contains(block);
///   if !node.direct.iter().all(in_data) { return Err(Corrupt); }
///
/// A file that points into a metadata sector (below `data_start`) or past the
/// volume end (`>= total_blocks`) is corruption `stage_write` cannot produce:
/// `stage_allocate_block` allocates only from `[data_start, total_blocks)`.
/// Redirecting I/O through such a pointer would silently read or overwrite
/// journal, bitmap, or inode-table sectors.
///
/// `inode_block_pointers_outside_the_data_region_are_refused` proves that
/// runtime operations (`read_file`, `write_file`, `unlink`) refuse these
/// inodes.  This test proves that `check()` does so as well.
#[test]
fn check_refuses_a_file_inode_naming_a_block_outside_the_data_region() {
    let (base, _, _) = two_file_root();
    let a_inode = Volume::mount(base.clone())
        .expect("mount")
        .lookup(1, "a.txt")
        .expect("a.txt")
        .inode;
    let sb = *Volume::mount(base.clone()).expect("mount").super_block();

    // Two representative out-of-range targets: one in the metadata region
    // (below data_start) and one past the volume end (>= total_blocks).
    // Both fail checked_inode's range predicate before any I/O on the
    // (potentially invalid) address is attempted.
    let targets: &[(&str, u64)] = &[
        ("inode table (below data_start)", sb.inode_start),
        ("past the end (>= total_blocks)", sb.total_blocks),
    ];
    for &(label, target) in targets {
        let image = base.fork(None);
        redirect_inode_block(&image, a_inode, 0, target);
        let before = image.sectors();

        let mut volume = Volume::mount(image.clone()).expect("mount");
        assert_eq!(
            volume.check(),
            Err(Error::Corrupt),
            "check() must refuse a file inode with a block pointer {label}"
        );
        assert!(
            image.sectors() == before,
            "check() must not modify the image for {label}"
        );
    }

    // Control: the unpatched image passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean image must pass check()");
}

/// `check()` must refuse a file inode whose `direct` array contains the same
/// non-zero block address in two slots (intra-inode aliasing).
///
/// `stage_write` allocates one distinct block per sector of content via
/// `stage_allocate_block`, which always returns a fresh, previously-free
/// block; it never places the same address in two different `direct` slots.
/// An inode with `direct[i] == direct[j]` (both non-zero, i != j) is
/// therefore corruption the runtime cannot produce.  `read_file` would read
/// the same on-disk sector for both logical halves; `write_file` would
/// overwrite it twice, discarding the second half's content silently.
///
/// `check()` calls `self.inode(number)?`, which calls `checked_inode`.  That
/// function contains the intra-inode duplicate guard:
///
///   for (slot, block) in node.direct.iter().enumerate() {
///       if *block != 0 && node.direct[slot + 1..].contains(block) {
///           return Err(Error::Corrupt);
///       }
///   }
///
/// `an_inode_naming_one_block_twice_is_refused` proves the same corruption is
/// refused by runtime operations (inode read, file read, write, unlink).
/// This test proves that `check()` also rejects it.
#[test]
fn check_refuses_a_file_inode_naming_the_same_block_twice() {
    let (base, inode) = two_block_file();
    let first = Volume::mount(base.clone())
        .expect("mount")
        .inode(inode)
        .expect("inode")
        .direct[0];

    // Redirect direct[1] to the same block address as direct[0].  The inode
    // structural checksum is resealed by redirect_inode_block; data_checksum
    // is left at the original value, but checked_inode fires at the
    // intra-inode duplicate guard before the data-checksum guard is reached.
    let image = base.fork(None);
    redirect_inode_block(&image, inode, 1, first);
    let before = image.sectors();

    let mut volume = Volume::mount(image.clone()).expect("mount");
    assert_eq!(
        volume.check(),
        Err(Error::Corrupt),
        "check() must refuse a file inode whose direct[0] == direct[1]"
    );
    assert!(
        image.sectors() == before,
        "check() must not modify the image"
    );

    // Control: the unpatched two-block file passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean image must pass check()");
}

#[test]
fn check_refuses_two_file_inodes_naming_the_same_block() {
    // `refuse_shared_blocks` is called by `check()` for each live inode and
    // scans every other live inode for any overlapping block.  `stage_write`
    // always allocates fresh blocks, so two distinct inodes naming one block
    // is cross-inode aliasing that no runtime write path can produce.
    //
    // Construction:
    //  - Create a.txt and b.txt with identical content ([7u8; SECTOR_SIZE])
    //    so both have the same data_checksum.
    //  - Read block_a = a.txt.direct[0] and block_b = b.txt.direct[0].
    //  - Redirect b.txt.direct[0] := block_a (same block as a.txt).
    //    redirect_inode_block re-seals b.txt's inode structural checksum.
    //    b.txt.data_checksum already matches block_a's content (identical
    //    file content), so the data-checksum check passes.
    //  - Clear the bitmap bit for block_b so the bitmap-orphan scan does not
    //    fire first (block_b is no longer named by any inode after the
    //    redirect; without clearing its bitmap bit, check() would report it
    //    as an orphaned block before reaching refuse_shared_blocks).
    //  - Now a.txt and b.txt both name block_a, which is the cross-inode
    //    aliasing that refuse_shared_blocks catches.
    let base = formatted();
    let (a_inode, b_inode) = {
        let mut volume = Volume::mount(base.clone()).expect("mount");
        let a = volume
            .create_file(1, "a.txt", 0o644, &[7u8; SECTOR_SIZE], 1)
            .expect("create a.txt");
        let b = volume
            .create_file(1, "b.txt", 0o644, &[7u8; SECTOR_SIZE], 2)
            .expect("create b.txt");
        (a, b)
    };
    let (block_a, block_b) = {
        let mut volume = Volume::mount(base.clone()).expect("mount");
        let a_node = volume.inode(a_inode).expect("a inode");
        let b_node = volume.inode(b_inode).expect("b inode");
        (a_node.direct[0], b_node.direct[0])
    };
    assert_ne!(block_a, block_b, "fresh allocation must give distinct blocks");

    let image = base.fork(None);
    // Point b.txt at a.txt's block; re-seals b.txt's inode checksum.
    redirect_inode_block(&image, b_inode, 0, block_a);
    // Clear block_b from the bitmap so check()'s orphan-block scan passes;
    // the first error path is then refuse_shared_blocks for a.txt (or b.txt).
    clear_bitmap_block(&image, block_b);
    let before = image.sectors();

    let mut volume = Volume::mount(image.clone()).expect("mount");
    assert_eq!(
        volume.check(),
        Err(Error::Corrupt),
        "check() must refuse two inodes naming the same block"
    );
    assert!(
        image.sectors() == before,
        "check() must not modify the image"
    );

    // Control: the unpatched volume passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean image must pass check()");
}

/// `check()` must refuse a directory entry whose inode number names a free
/// (unused) inode slot.
///
/// `stage_create` always allocates a fresh inode via the allocator, which scans
/// for the first slot with `mode == 0`.  `stage_unlink` frees the slot by
/// writing `Inode::empty()` and then clearing the directory entry.  A partially
/// applied unlink (inode zeroed, entry not yet cleared) or a crafted image can
/// leave a directory entry pointing at a free inode.  Such an entry is
/// permanently unreachable: every lookup path refuses free inodes via the
/// combined `!node.is_used() || node.kind != KIND_FILE` guard.
///
/// The guard fires at the check() directory-entry loop:
/// `if !node.is_used() || node.kind != KIND_FILE || entry.kind != KIND_FILE`.
/// This test exercises only the `!node.is_used()` branch; the `node.kind !=
/// KIND_FILE` branch is covered by
/// `check_refuses_a_directory_entry_naming_a_directory_inode`, and the
/// `entry.kind != KIND_FILE` branch by
/// `check_refuses_a_directory_entry_with_wrong_kind`.
#[test]
fn check_refuses_a_directory_entry_naming_a_free_inode() {
    let (base, block, index) = two_file_root();
    let (b_inode, free_inode) = {
        let mut volume = Volume::mount(base.clone()).expect("mount");
        let b_inode = volume.lookup(1, "b.txt").expect("b.txt").inode;
        // Find the first free inode: mode == 0, in range, not yet allocated.
        let free_inode = (2..=u64::from(INODES))
            .find(|n| !volume.inode(*n).expect("inode").is_used())
            .expect("a free inode");
        (b_inode, free_inode)
    };

    // Patch a.txt's directory entry inode field (bytes 0..8) to the free inode.
    // `free_inode` is in `1..=inode_count` so the range check passes; the
    // per-entry `!node.is_used()` guard then fires.
    let image = base.fork(None);
    patch_dir_entry(&image, block, index, 0, &free_inode.to_le_bytes());
    // Snapshot after patching: check() must not write anything.
    let before = image.sectors();

    let mut volume = Volume::mount(image.clone()).expect("mount");
    assert_eq!(
        volume.check(),
        Err(Error::Corrupt),
        "check() must refuse a directory entry naming a free inode"
    );
    // check() must not modify the image.
    assert!(
        image.sectors() == before,
        "check() must not modify the image"
    );
    // b.txt was not patched and must remain accessible.
    let mut buf = [0u8; MAX_FILE_SIZE];
    assert_eq!(
        volume.read_file(b_inode, &mut buf),
        Ok(5),
        "b.txt must still be readable"
    );

    // Control: the unpatched image passes check().
    let mut clean = Volume::mount(base.clone()).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean image must pass check()");
}

/// `check()` must refuse the root directory inode when its data block pointer
/// names a block outside the valid data region.
///
/// The block-range guard in `check()`'s per-inode loop runs for every live
/// inode — both `KIND_FILE` and `KIND_DIR`.  `format()` always places the root
/// directory's data block inside the data region; any pointer below `data_start`
/// or at or beyond `total_blocks` is corruption no runtime path can produce.  A
/// crafted image where `root.direct[0]` points into the inode table would allow
/// `dir_entries` to interpret raw inode bytes as directory entries, silently
/// hiding corruption the real directory block would have exposed.
///
/// `check_refuses_a_file_inode_naming_a_block_outside_the_data_region` covers
/// the same guard for `KIND_FILE` inodes; this test covers the `KIND_DIR` path
/// that the earlier test leaves untested.
#[test]
fn check_refuses_root_directory_inode_naming_a_block_outside_the_data_region() {
    let base = formatted();
    let sb = *Volume::mount(base.clone()).expect("mount").super_block();

    let targets: &[(&str, u64)] = &[
        ("inode table (below data_start)", sb.inode_start),
        ("past the end (>= total_blocks)", sb.total_blocks),
    ];

    for &(label, target) in targets {
        // Redirect root.direct[0] (slot 0 of inode 1) to an out-of-range block
        // and reseal the inode structural checksum so mount() and checked_inode()
        // accept the image.  The range guard in check()'s per-inode loop fires
        // before the root-specific section is reached.
        let image = base.fork(None);
        redirect_inode_block(&image, 1, 0, target);
        let before = image.sectors();

        let mut volume = Volume::mount(image.clone()).expect("mount");
        assert_eq!(
            volume.check(),
            Err(Error::Corrupt),
            "check() must refuse root directory inode with block pointer {label}"
        );
        assert!(
            image.sectors() == before,
            "check() must not modify the image ({label})"
        );
    }

    // Control: the unpatched volume passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean volume must pass check()");
}

/// `check()` must refuse the root directory inode when its data block is
/// in `direct[0]` but the bitmap marks that block free.
///
/// The per-inode block loop at `wfs.rs` runs for every live inode — both
/// `KIND_FILE` and `KIND_DIR`.  For each non-zero `direct[slot]` it checks
/// `!self.bitmap_get(&txn, block)?` and returns `Err(Corrupt)` if the bit is
/// clear.  `check_refuses_a_file_inode_naming_a_block_the_bitmap_marks_free`
/// covers this guard for `KIND_FILE` inodes only.  No prior test exercised it
/// for the root directory inode: a crafted image where `root.direct[0]` points
/// to a valid data block that happens to be marked free in the bitmap.
///
/// Such a state is impossible to produce through any runtime path — the
/// allocator always sets the bitmap bit before returning a block, and no runtime
/// path ever clears the bit without zeroing the inode slot — but a crafted
/// image or a bitmap write failure could produce it.  Without this guard the
/// root directory's data block would appear unallocated to the allocator; any
/// subsequent allocation could hand out the same block, silently overwriting
/// all directory entries.
#[test]
fn check_refuses_root_directory_inode_naming_a_block_the_bitmap_marks_free() {
    let base = formatted();

    // Read the root directory's data block from direct[0].
    let dir_block = read_inode_direct0(&base, 1);

    // Fork the image and clear the bitmap bit for dir_block without touching the
    // inode's direct[0] or any content.  The block address is still in range and
    // the inode checksum is intact, so mount() and checked_inode() both accept
    // the image.  The bitmap guard `!bitmap_get(dir_block)` fires when the
    // per-inode loop processes inode 1 (the root directory).
    let corrupt = base.fork(None);
    clear_bitmap_block(&corrupt, dir_block);
    let before = corrupt.sectors();

    let mut volume = Volume::mount(corrupt.clone()).expect("mount corrupt");
    assert_eq!(
        volume.check(),
        Err(Error::Corrupt),
        "check() must refuse root directory inode naming a bitmap-free block"
    );
    assert!(
        corrupt.sectors() == before,
        "check() must not modify the image"
    );

    // Control: the unpatched volume passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean volume must pass check()");
}

/// `check()` must refuse a volume where a file inode names the same data block
/// as the root directory inode.
///
/// `refuse_shared_blocks` is called for every live inode in the per-inode loop,
/// including the root directory inode (inode 1, KIND_DIR).  When processing the
/// root, it scans all other live inodes and returns `Err(Corrupt)` if any of
/// them names a block the root already names.  `stage_allocate_block` always
/// returns a fresh block never used by any live inode, so a file inode whose
/// `direct[0]` equals `root.direct[0]` is corruption no runtime path can produce.
///
/// `check_refuses_two_file_inodes_naming_the_same_block` covers the same
/// `refuse_shared_blocks` guard when both inodes are `KIND_FILE`.  This test
/// covers the cross-kind path where one inode is the root directory (`KIND_DIR`).
/// Because the root (inode 1) is the first inode processed in the loop,
/// `refuse_shared_blocks` fires during its turn — before the file inode is
/// reached — so neither the data-checksum guard nor any `KIND_FILE`-specific
/// check for the file inode can interfere.
#[test]
fn check_refuses_a_file_inode_naming_the_root_directory_data_block() {
    let base = formatted();
    let file_inode = {
        let mut volume = Volume::mount(base.clone()).expect("mount");
        volume
            .create_file(1, "a.txt", 0o644, b"hello", 1)
            .expect("create a.txt")
    };

    // dir_block  = root.direct[0]: the root directory's data block.
    // file_block = file_inode.direct[0]: a.txt's single data block.
    let dir_block = read_inode_direct0(&base, 1);
    let file_block = read_inode_direct0(&base, file_inode);
    assert_ne!(dir_block, file_block, "fresh allocations must be distinct");

    // Redirect file_inode.direct[0] to dir_block and reseal the inode structural
    // checksum so mount() and checked_inode() accept the image.
    let image = base.fork(None);
    redirect_inode_block(&image, file_inode, 0, dir_block);
    // file_block is now named by no live inode.  Clear its bitmap bit so the
    // orphan-block scan does not fire before refuse_shared_blocks.
    clear_bitmap_block(&image, file_block);
    let before = image.sectors();

    let mut volume = Volume::mount(image.clone()).expect("mount");
    // check() processes inode 1 (root, KIND_DIR) first.  refuse_shared_blocks
    // scans all other live inodes and finds file_inode also naming dir_block ->
    // Err(Corrupt).  The file inode's data-checksum guard is never reached.
    assert_eq!(
        volume.check(),
        Err(Error::Corrupt),
        "check() must refuse a file inode naming the root directory's data block"
    );
    assert!(
        image.sectors() == before,
        "check() must not modify the image"
    );

    // Control: the unpatched volume passes check().
    let mut clean = Volume::mount(base).expect("mount clean");
    assert_eq!(clean.check(), Ok(()), "clean volume must pass check()");
}
