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
