//! Kernel-authorized durable storage for the WovenIdentity snapshot.
//!
//! The identity policy core owns encoding and authentication. This adapter
//! adds two native WovenFS copies with a generation and wrapper checksum; a
//! caller must already hold the kernel-only StorageIo authority. Sessions are
//! never written to disk.

use libwoven::identity::{Authority, Error as IdentityError, SNAPSHOT_BYTES};

const PATHS: [&str; 2] = ["/wfs/identity-state-a", "/wfs/identity-state-b"];
const RECORD_BYTES: usize = 4 + 8 + SNAPSHOT_BYTES + 4;
const MAGIC: [u8; 4] = *b"WID2";

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecordError {
    Invalid,
    Storage,
}

fn checksum(bytes: &[u8]) -> u32 {
    let mut value = 0x811c9dc5u32;
    for byte in bytes {
        value ^= u32::from(*byte);
        value = value.wrapping_mul(0x01000193);
    }
    value
}

fn encode(sequence: u64, snapshot: &[u8; SNAPSHOT_BYTES], output: &mut [u8; RECORD_BYTES]) {
    output[..4].copy_from_slice(&MAGIC);
    output[4..12].copy_from_slice(&sequence.to_le_bytes());
    output[12..12 + SNAPSHOT_BYTES].copy_from_slice(snapshot);
    let value = checksum(&output[..RECORD_BYTES - 4]);
    output[RECORD_BYTES - 4..].copy_from_slice(&value.to_le_bytes());
}

fn decode(input: &[u8; RECORD_BYTES]) -> Option<(u64, [u8; SNAPSHOT_BYTES])> {
    if input[..4] != MAGIC
        || checksum(&input[..RECORD_BYTES - 4])
            != u32::from_le_bytes(input[RECORD_BYTES - 4..].try_into().ok()?)
    {
        return None;
    }
    let sequence = u64::from_le_bytes(input[4..12].try_into().ok()?);
    if sequence == 0 {
        return None;
    }
    let mut snapshot = [0u8; SNAPSHOT_BYTES];
    snapshot.copy_from_slice(&input[12..12 + SNAPSHOT_BYTES]);
    Authority::decode_snapshot(&snapshot).ok()?;
    Some((sequence, snapshot))
}

fn read_copy(index: usize, output: &mut [u8; RECORD_BYTES]) -> Result<usize, RecordError> {
    crate::wfs::read_vfs_path_durable(PATHS[index], output).map_err(|_| RecordError::Storage)
}

pub fn persist(
    authority: &crate::wfs::AuthorizedWfsWriter,
    snapshot: &[u8; SNAPSHOT_BYTES],
    now: u64,
) -> Result<u64, RecordError> {
    Authority::decode_snapshot(snapshot).map_err(|_| RecordError::Invalid)?;
    let mut first = [0u8; RECORD_BYTES];
    let mut second = [0u8; RECORD_BYTES];
    let left = read_copy(0, &mut first).ok().and_then(|length| {
        (length == RECORD_BYTES).then(|| decode(&first)).flatten()
    });
    let right = read_copy(1, &mut second).ok().and_then(|length| {
        (length == RECORD_BYTES).then(|| decode(&second)).flatten()
    });
    let sequence = left
        .map_or(0, |record| record.0)
        .max(right.map_or(0, |record| record.0))
        .checked_add(1)
        .ok_or(RecordError::Invalid)?;
    let mut record = [0u8; RECORD_BYTES];
    encode(sequence, snapshot, &mut record);
    crate::wfs::write_vfs_path_durable(authority, PATHS[(sequence as usize) & 1], &record, now)
        .map_err(|_| RecordError::Storage)?;
    Ok(sequence)
}

pub fn load(output: &mut [u8; SNAPSHOT_BYTES]) -> Result<u64, RecordError> {
    let mut first = [0u8; RECORD_BYTES];
    let mut second = [0u8; RECORD_BYTES];
    let left = read_copy(0, &mut first).ok().and_then(|length| {
        (length == RECORD_BYTES).then(|| decode(&first)).flatten()
    });
    let right = read_copy(1, &mut second).ok().and_then(|length| {
        (length == RECORD_BYTES).then(|| decode(&second)).flatten()
    });
    let record = match (left, right) {
        (Some(left), Some(right)) if left.0 > right.0 => left,
        (Some(left), Some(right)) if right.0 > left.0 => right,
        (Some(_), Some(_)) => return Err(RecordError::Invalid),
        (Some(record), None) | (None, Some(record)) => record,
        (None, None) => return Err(RecordError::Storage),
    };
    output.copy_from_slice(&record.1);
    Ok(record.0)
}

pub fn durable_self_test() -> bool {
    fn fail(reason: &str) -> bool {
        crate::serial::write_line(format_args!("[S24.3] {}", reason));
        false
    }
    if !crate::wfs::format_ata(64) {
        return fail("format failed");
    }
    let Some(authority) = crate::update::UpdateState::authorized_writer() else {
        return fail("storage authority unavailable");
    };
    let mut policy = Authority::new();
    let Ok(group) = policy.add_group(b"users") else {
        return fail("group creation failed");
    };
    if policy
        .add_administrator(b"alice", group, b"qemu-salt", b"1234")
        .is_err()
    {
        return fail("administrator creation failed");
    }
    let mut snapshot = [0u8; SNAPSHOT_BYTES];
    if policy.encode_snapshot(&mut snapshot).is_err()
        || persist(&authority, &snapshot, 1) != Ok(1)
    {
        return fail("initial snapshot write failed");
    }
    let mut restored = [0u8; SNAPSHOT_BYTES];
    if load(&mut restored) != Ok(1) || restored != snapshot {
        return fail("initial snapshot recovery failed");
    }
    if persist(&authority, &snapshot, 2) != Ok(2) || load(&mut restored) != Ok(2) {
        return fail("generation update failed");
    }
    if Authority::decode_snapshot(&restored).is_err() {
        return fail("recovered snapshot rejected");
    }
    true
}
