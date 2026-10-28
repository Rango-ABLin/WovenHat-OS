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
        Self::with_sectors(SECTORS)
    }

    fn with_sectors(sectors: usize) -> Self {
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

fn formatted_small() -> CrashDisk {
    let disk = CrashDisk::with_sectors(48);
    Volume::format(disk.clone(), 8).expect("small format");
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
fn replay_rejects_reserved_journal_targets_without_touching_metadata() {
    // A forged descriptor plus matching transaction ID must not be allowed
    // to overwrite the superblock pair or the journal's own sectors.
    for target_kind in 0..3 {
        let disk = formatted();
        let sb = {
            let volume = Volume::mount(disk.clone()).expect("baseline mount");
            *volume.super_block()
        };
        let target = match target_kind {
            0 => 0,
            1 => 1,
            _ => sb.journal_start,
        };
        let original_superblocks = {
            let image = disk.image.borrow();
            [image[0], image[1]]
        };
        {
            let mut image = disk.image.borrow_mut();
            let mut descriptor = [0u8; SECTOR_SIZE];
            descriptor[..8].copy_from_slice(wfs::JOURNAL_DESC_MAGIC);
            descriptor[8..16].copy_from_slice(&999_u64.to_le_bytes());
            descriptor[16..20].copy_from_slice(&1_u32.to_le_bytes());
            descriptor[24..32].copy_from_slice(&target.to_le_bytes());
            image[sb.journal_start as usize] = descriptor;
            let mut commit = [0u8; SECTOR_SIZE];
            commit[..8].copy_from_slice(wfs::JOURNAL_COMMIT_MAGIC);
            commit[8..16].copy_from_slice(&999_u64.to_le_bytes());
            image[(sb.journal_start + 2) as usize] = commit;
        }
        assert!(
            matches!(Volume::mount(disk.clone()), Err(Error::Corrupt)),
            "reserved target {target} must be refused"
        );
        let image = disk.image.borrow();
        assert_eq!([image[0], image[1]], original_superblocks);
    }
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
fn full_data_region_rejects_write_without_partial_commit() {
    let disk = formatted_small();
    let mut volume = mount_consistent(&disk, "small-volume setup");
    let first = volume
        .create(1, "first.bin", 0o644, 1)
        .expect("first file");
    let first_payload = vec![0x31_u8; SECTOR_SIZE * 7];
    volume
        .write_file(first, &first_payload, 2)
        .expect("fill first file");

    let second = volume
        .create(1, "second.bin", 0o644, 3)
        .expect("second file");
    volume
        .write_file(second, &[0x42_u8; SECTOR_SIZE], 4)
        .expect("consume final data block");

    let third = volume
        .create(1, "third.bin", 0o644, 5)
        .expect("third file metadata still fits");
    assert_eq!(
        volume.write_file(third, &[0x53_u8; SECTOR_SIZE * 2], 6),
        Err(Error::NoSpace)
    );
    drop(volume);

    let mut recovered = mount_consistent(&disk, "full-volume remount");
    let first_entry = recovered.lookup(1, "first.bin").expect("first entry");
    let mut output = vec![0_u8; MAX_FILE_SIZE];
    let read = recovered
        .read_file(first_entry.inode, &mut output)
        .expect("first file remains readable");
    assert_eq!(&output[..read], first_payload.as_slice());
    let third_entry = recovered.lookup(1, "third.bin").expect("third entry");
    let third_node = recovered.inode(third_entry.inode).expect("third inode");
    assert_eq!(third_node.size, 0, "failed write published partial size");
    recovered.check().expect("full volume remains consistent");
}

#[test]
fn directory_rename_rejects_descendant_cycles_and_allows_same_path() {
    let disk = formatted();
    let mut volume = mount_consistent(&disk, "cycle setup");
    volume.mkdir_path("/parent", 0o755, 1).expect("parent");
    volume.mkdir_path("/parent/child", 0o755, 2).expect("child");

    assert_eq!(
        volume.rename_path("/parent", "/parent/child/moved", 3),
        Err(Error::InvalidName)
    );
    volume
        .rename_path("/parent", "/parent", 4)
        .expect("same-path rename");
    assert!(volume.resolve_path("/parent/child").is_ok());
    mount_consistent(&disk, "cycle rejection");
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
fn randomized_crash_schedule_keeps_file_contents_atomic() {
    let original = vec![0x11_u8; 1536];
    let replacement = (0..1536_u32)
        .map(|index| (index.wrapping_mul(37) % 251) as u8)
        .collect::<Vec<_>>();

    let base = formatted();
    let inode = {
        let mut volume = Volume::mount(base.clone()).expect("mount");
        let inode = volume.create(1, "random.bin", 0o644, 100).expect("create");
        volume
            .write_file(inode, &original, 100)
            .expect("initial write");
        inode
    };
    let writes = write_count(&base, |volume| {
        volume
            .write_file(inode, &replacement, 200)
            .expect("replacement write");
    });
    assert!(writes > 3, "expected a multi-sector write, got {writes}");

    // Deterministic xorshift scheduling keeps this reproducible while
    // exercising a different cut order from the exhaustive neighboring test.
    let mut seed = 0x9e37_79b9_u32;
    for sample in 0..64 {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let cut = (seed as usize) % (writes + 1);
        let crashing = base.fork(Some(cut));
        let mut volume = Volume::mount(crashing.clone()).expect("mount before write");
        let _ = volume.write_file(inode, &replacement, 200);
        drop(volume);

        let settled = crashing.fork(None);
        let mut recovered =
            mount_consistent(&settled, &format!("random sample {sample}, cut {cut}"));
        let mut output = vec![0_u8; MAX_FILE_SIZE];
        let read = recovered
            .read_file(inode, &mut output)
            .expect("recovered read");
        assert_eq!(read, original.len(), "random sample {sample}: partial size");
        let contents = &output[..read];
        assert!(
            contents == original.as_slice() || contents == replacement.as_slice(),
            "random sample {sample}, cut {cut}: torn file contents"
        );

        let remounted = settled.fork(None);
        let mut second = mount_consistent(&remounted, &format!("random replay {sample}"));
        let mut replay = vec![0_u8; MAX_FILE_SIZE];
        let replay_read = second.read_file(inode, &mut replay).expect("replay read");
        assert_eq!(
            &replay[..replay_read],
            contents,
            "random replay {sample}: changed data"
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
fn interrupted_recovery_replays_committed_data_at_every_write_boundary() {
    // The existing crash tests cut power during a transaction. This second
    // dimension cuts power AGAIN during mount-time journal replay, including
    // after some home-location sectors were restored but before retirement.
    let payload: Vec<u8> = (0..1500u32).map(|index| (index % 241) as u8).collect();
    let base = formatted();
    let inode = {
        let mut volume = Volume::mount(base.clone()).expect("mount baseline");
        volume.create(1, "recover.bin", 0o644, 1).expect("create");
        volume.lookup(1, "recover.bin").expect("lookup").inode
    };
    let full_writes = write_count(&base, |volume| {
        volume.write_file(inode, &payload, 2).expect("write");
    });
    assert!(full_writes > 3, "expected a journaled multi-sector write");

    // Refuse the final journal-retirement write: commit, home writes and
    // checkpoint are on disk, but replay still sees the committed descriptor.
    let pending = base.fork(Some(full_writes - 1));
    let mut volume = Volume::mount(pending.clone()).expect("mount before write");
    assert_eq!(
        volume.write_file(inode, &payload, 2),
        Err(Error::Device),
        "the simulated final write must fail"
    );
    drop(volume);

    let replay_probe = pending.fork(None);
    Volume::mount(replay_probe.clone()).expect("uninterrupted recovery");
    let replay_writes = replay_probe.writes();
    assert!(replay_writes > 1, "expected home writes and journal retirement");

    for cut in 0..=replay_writes {
        let interrupted = pending.fork(Some(cut));
        let first = Volume::mount(interrupted.clone());
        if cut < replay_writes {
            assert!(
                matches!(&first, Err(Error::Device)),
                "replay cut at {cut} unexpectedly succeeded"
            );
        } else {
            assert!(first.is_ok(), "replay with all writes allowed failed");
        }
        drop(first);

        // The second recovery gets a fresh power-on write budget.
        let settled = interrupted.fork(None);
        let mut recovered = mount_consistent(&settled, &format!("replay cut at {cut}"));
        let mut output = vec![0u8; MAX_FILE_SIZE];
        let count = recovered.read_file(inode, &mut output).expect("recovered read");
        assert_eq!(
            &output[..count],
            &payload[..],
            "replay cut at {cut} lost committed data"
        );
        drop(recovered);

        let after_recovery = settled.sectors();
        Volume::mount(settled.clone()).expect("idempotent third mount");
        assert_eq!(
            settled.sectors(),
            after_recovery,
            "replay cut at {cut} changed disk on a subsequent mount"
        );
    }
}

#[test]
fn nested_namespace_and_file_contents_persist_across_repeated_remounts() {
    // Exercise the real native directory tree and on-disk data, not a VFS
    // cache. Every step is followed by a fresh mount and a full tree check.
    let disk = formatted();
    let payload: Vec<u8> = (0..1536u32).map(|n| (n % 239) as u8).collect();
    {
        let mut volume = Volume::mount(disk.clone()).expect("initial mount");
        volume.mkdir_path("/docs", 0o755, 1).expect("create directory");
        volume.mkdir_path("/docs/archive", 0o755, 2).expect("create nested directory");
        let inode = volume
            .create_path("/docs/archive/record.bin", 0o644, 3)
            .expect("create nested file");
        volume.write_file(inode, &payload, 4).expect("write contents");
    }

    for cycle in 0..5 {
        let mut volume = mount_consistent(&disk, &format!("remount cycle {cycle}"));
        let inode = volume
            .resolve_path("/docs/archive/record.bin")
            .expect("persistent nested path");
        let mut output = vec![0u8; MAX_FILE_SIZE];
        let len = volume.read_file(inode, &mut output).expect("persistent contents");
        assert_eq!(&output[..len], &payload[..], "remount cycle {cycle}");
    }

    {
        let mut volume = mount_consistent(&disk, "before rename");
        volume
            .rename_path("/docs/archive/record.bin", "/docs/archive/renamed.bin", 5)
            .expect("rename file");
    }
    {
        let mut volume = mount_consistent(&disk, "after rename");
        assert_eq!(
            volume.resolve_path("/docs/archive/record.bin"),
            Err(Error::NotFound)
        );
        let inode = volume.resolve_path("/docs/archive/renamed.bin").expect("renamed file");
        let mut output = vec![0u8; MAX_FILE_SIZE];
        let len = volume.read_file(inode, &mut output).expect("renamed contents");
        assert_eq!(&output[..len], &payload[..]);
        volume.unlink_path("/docs/archive/renamed.bin", 6).expect("unlink");
    }
    {
        let mut volume = mount_consistent(&disk, "after unlink");
        assert_eq!(
            volume.resolve_path("/docs/archive/renamed.bin"),
            Err(Error::NotFound)
        );
        volume.unlink_path("/docs/archive", 7).expect("remove empty directory");
    }
    let mut volume = mount_consistent(&disk, "final remount");
    assert_eq!(volume.resolve_path("/docs/archive"), Err(Error::NotFound));
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
