//! Privileged WovenUpdate transaction boundary.
//!
//! This service is deliberately fail-closed.  It verifies the signed WUPD
//! envelope before changing metadata, writes the inactive slot before
//! publishing the corresponding WUAB record, and selects the alternate record
//! copy for every commit.  The current WovenFS file limit is smaller than the
//! package format's maximum image size, so images that cannot be stored in one
//! durable slot file are rejected rather than partially installed.

use crate::update::{
    Error as UpdateError, UpdateState, UPDATE_PACKAGE_HEADER_LEN, UPDATE_SLOT_PATHS,
};

pub const OP_STAGE: u8 = 1;
pub const OP_ACTIVATE: u8 = 2;
pub const OP_BOOT_SUCCEEDED: u8 = 3;
pub const OP_BOOT_FAILED: u8 = 4;
pub const OP_QUERY: u8 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Capability,
    InvalidSource,
    Storage,
    Uninitialized,
    InvalidRecord,
    Package(UpdateError),
    ImageTooLarge,
    MissingCandidate,
    InvalidOperation,
}

struct Snapshot {
    sequence: u64,
    copy: usize,
    state: UpdateState,
}

fn authority() -> Result<crate::wfs::AuthorizedWfsWriter, Error> {
    UpdateState::authorized_writer().ok_or(Error::Capability)
}

fn load() -> Result<Snapshot, Error> {
    let mut records = [[0u8; crate::update::UPDATE_RECORD_LEN]; 2];
    let mut decoded = [None; 2];
    let mut present = 0;
    for copy in 0..2 {
        match UpdateState::read_record_copy(copy, &mut records[copy]) {
            Ok(length) if length == crate::update::UPDATE_RECORD_LEN => {
                present += 1;
                decoded[copy] = UpdateState::decode_record(&records[copy]).ok();
            }
            Err(UpdateError::RecordMissing) => {}
            Err(_) => return Err(Error::Storage),
            _ => return Err(Error::InvalidRecord),
        }
    }
    if present == 0 {
        return Err(Error::Uninitialized);
    }
    let (sequence, state) =
        UpdateState::select_record(&records[0], &records[1]).map_err(Error::Package)?;
    let copy = if decoded[0].is_some_and(|record| record.0 == sequence) {
        0
    } else if decoded[1].is_some_and(|record| record.0 == sequence) {
        1
    } else {
        return Err(Error::InvalidRecord);
    };
    Ok(Snapshot {
        sequence,
        copy,
        state,
    })
}

fn persist(snapshot: &Snapshot, state: &UpdateState, now: u64) -> Result<(), Error> {
    let sequence = snapshot
        .sequence
        .checked_add(1)
        .ok_or(Error::InvalidRecord)?;
    let copy = 1 - snapshot.copy;
    let writer = authority()?;
    UpdateState::persist_record_copy(&writer, copy, sequence, state, now).map_err(Error::Package)
}

fn validate_source(source: &str) -> Result<(), Error> {
    let bytes = source.as_bytes();
    if bytes.is_empty()
        || bytes.len() > libwoven::update_ipc::MAX_PATH_BYTES
        || !bytes.starts_with(b"/updates/")
        || bytes.windows(2).any(|window| window == b"..")
        || bytes.iter().any(|byte| {
            !matches!(*byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'/' | b'.' | b'_' | b'-')
        })
    {
        return Err(Error::InvalidSource);
    }
    Ok(())
}

/// Stage one already-buffered signed package. This is the kernel-internal
/// primitive used by the VFS source path and by the executable storage gate.
pub fn stage_package(package: &[u8], now: u64) -> Result<usize, Error> {
    let _ = authority()?;
    if package.len() > crate::wfs::MAX_FILE_SIZE {
        return Err(Error::ImageTooLarge);
    }
    let candidate = crate::update::verify_package(package).map_err(Error::Package)?;
    if candidate.image.len() > crate::wfs::MAX_FILE_SIZE {
        return Err(Error::ImageTooLarge);
    }
    let snapshot = load()?;
    let mut next = snapshot.state;
    let slot = next
        .stage_candidate(
            candidate.generation,
            candidate.image_length,
            candidate.digest,
        )
        .map_err(Error::Package)?;
    let writer = authority()?;
    crate::wfs::write_vfs_path_durable(&writer, UPDATE_SLOT_PATHS[slot], candidate.image, now)
        .map_err(|_| Error::Storage)?;
    persist(&snapshot, &next, now)?;
    Ok(slot)
}

pub fn stage_source(source: &str, now: u64) -> Result<usize, Error> {
    validate_source(source)?;
    let capacity = crate::wfs::MAX_FILE_SIZE;
    if capacity < UPDATE_PACKAGE_HEADER_LEN {
        return Err(Error::ImageTooLarge);
    }
    let mut package = crate::heap::try_filled(0u8, capacity).ok_or(Error::Storage)?;
    let length = crate::vfs::read_all(source, &mut package).map_err(|_| Error::Storage)?;
    stage_package(&package[..length], now)
}

pub fn activate(slot: usize, now: u64) -> Result<(), Error> {
    let snapshot = load()?;
    let mut next = snapshot.state;
    if next.slot(slot).is_none()
        || next
            .slot(slot)
            .is_some_and(|entry| entry.state == crate::update::SlotState::Empty)
    {
        return Err(Error::MissingCandidate);
    }
    next.activate_candidate(slot).map_err(Error::Package)?;
    persist(&snapshot, &next, now)
}

pub fn boot_succeeded(slot: usize, now: u64) -> Result<(), Error> {
    let snapshot = load()?;
    let mut next = snapshot.state;
    next.boot_succeeded(slot).map_err(Error::Package)?;
    persist(&snapshot, &next, now)
}

pub fn boot_failed(slot: usize, now: u64) -> Result<(), Error> {
    let snapshot = load()?;
    let mut next = snapshot.state;
    next.boot_failed(slot).map_err(Error::Package)?;
    persist(&snapshot, &next, now)
}

/// Return active slot in bits 0..=7 and pending slot plus one in bits 8..=15.
pub fn query() -> Result<u64, Error> {
    let snapshot = load()?;
    Ok(snapshot.state.active_slot() as u64
        | ((snapshot.state.pending_slot().map_or(0, |slot| slot + 1) as u64) << 8))
}

pub fn transact(operation: u8, source: Option<&str>, slot: usize, now: u64) -> Result<u64, Error> {
    match operation {
        OP_STAGE => Ok(stage_source(source.ok_or(Error::InvalidSource)?, now)? as u64),
        OP_ACTIVATE => {
            activate(slot, now)?;
            Ok(0)
        }
        OP_BOOT_SUCCEEDED => {
            boot_succeeded(slot, now)?;
            Ok(0)
        }
        OP_BOOT_FAILED => {
            boot_failed(slot, now)?;
            Ok(0)
        }
        OP_QUERY => query(),
        _ => Err(Error::InvalidOperation),
    }
}

#[cfg(feature = "stage27-5-test")]
pub fn self_test() -> bool {
    use ed25519_compact::{KeyPair, Seed};
    use sha2::{Digest, Sha256};

    if !crate::wfs::format_ata(64) {
        crate::serial::write_line(format_args!("[S27.5] format failed"));
        return false;
    }
    let Some(writer) = UpdateState::authorized_writer() else {
        crate::serial::write_line(format_args!("[S27.5] authority failed"));
        return false;
    };
    let Ok(initial) = UpdateState::new(1, 19, [1; 32]) else {
        return false;
    };
    if UpdateState::persist_record_copy(&writer, 0, 1, &initial, 1).is_err() {
        crate::serial::write_line(format_args!("[S27.5] initial record failed"));
        return false;
    }
    const SEED: [u8; 32] = [
        0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c,
        0xc4, 0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae,
        0x7f, 0x60,
    ];
    let image = b"signed-update-image";
    let mut package = [0u8; crate::update::UPDATE_PACKAGE_HEADER_LEN + 19];
    package[..4].copy_from_slice(&crate::update::UPDATE_PACKAGE_MAGIC);
    package[4..6].copy_from_slice(&crate::update::UPDATE_PACKAGE_VERSION.to_le_bytes());
    package[6..8].copy_from_slice(&(crate::update::UPDATE_PACKAGE_HEADER_LEN as u16).to_le_bytes());
    package[8..16].copy_from_slice(&2u64.to_le_bytes());
    package[16..24].copy_from_slice(&(image.len() as u64).to_le_bytes());
    let digest: [u8; 32] = Sha256::digest(image).into();
    package[24..56].copy_from_slice(&digest);
    package[crate::update::UPDATE_PACKAGE_HEADER_LEN..].copy_from_slice(image);
    let mut signed = Sha256::new();
    signed.update(b"WOVENHAT-UPDATE-V1");
    signed.update(&package[..56]);
    signed.update(image);
    let signature = KeyPair::from_seed(Seed::new(SEED))
        .sk
        .sign(signed.finalize(), None);
    package[56..120].copy_from_slice(&*signature);
    let Ok(slot) = stage_package(&package, 2) else {
        crate::serial::write_line(format_args!("[S27.5] stage failed"));
        return false;
    };
    let activated = activate(slot, 3);
    let observed = query();
    if activated.is_err() || observed != Ok(((slot + 1) as u64) << 8) {
        crate::serial::write_line(format_args!(
            "[S27.5] activate={:?} query={:?} slot={}",
            activated, observed, slot
        ));
        crate::serial::write_line(format_args!("[S27.5] activate/query failed"));
        return false;
    }
    if boot_succeeded(slot, 4).is_err() || query() != Ok(slot as u64) {
        crate::serial::write_line(format_args!("[S27.5] commit/query failed"));
        return false;
    }
    let mut image_read = [0u8; crate::wfs::MAX_FILE_SIZE];
    let read_ok = crate::wfs::read_vfs_path_durable(UPDATE_SLOT_PATHS[slot], &mut image_read)
        == Ok(image.len());
    if !read_ok {
        crate::serial::write_line(format_args!("[S27.5] slot read failed"));
    }
    read_ok && &image_read[..image.len()] == image
}

/// Exercise the signed Ring-3 WovenUpdate daemon through its real service
/// endpoint. The direct transaction self-test above establishes the durable
/// record and candidate state; this acceptance proves the userspace decode,
/// capability-gated query, malformed-request response, and teardown path.
#[cfg(feature = "stage27-5-test")]
pub fn service_ipc_self_test(update_image: &[u8]) -> bool {
    use libwoven::update_ipc::{Operation, Request, Response, NO_SLOT};

    const STATUS_OK: u16 = 0;
    const STATUS_BAD_REQUEST: u16 = 1;

    fn fail(reason: &str) -> bool {
        crate::serial::write_line(format_args!("[S27.5] IPC {}", reason));
        false
    }

    fn reap(pid: crate::task::ProcessId) -> bool {
        let _ = crate::task::kill_process(pid.as_u64(), 9);
        let deadline = crate::timer::ticks().wrapping_add(500);
        while !crate::task::process_exited(pid) {
            if crate::timer::ticks().wrapping_sub(deadline) < u64::MAX / 2 {
                return false;
            }
            crate::task::yield_now();
        }
        crate::task::wait_process(pid.as_u64()).is_ok()
    }

    fn exchange(
        pid: crate::task::ProcessId,
        handle: crate::ipc::Handle,
        bytes: &[u8],
        expected: Response,
    ) -> bool {
        if crate::ipc::send_handle(pid.as_u64(), handle, bytes).is_err() {
            crate::serial::write_line(format_args!("[S27.5] IPC send failed"));
            return false;
        }
        let deadline = crate::timer::ticks().wrapping_add(500);
        loop {
            if crate::task::process_exited(pid) {
                crate::serial::write_line(format_args!(
                    "[S27.5] IPC daemon exited before response"
                ));
                return false;
            }
            if let Ok((message, serial)) = crate::ipc::peek(pid.as_u64()) {
                let decoded = Response::decode(message.payload()).ok();
                let _ = crate::ipc::receive_if(pid.as_u64(), serial);
                if decoded != Some(expected) {
                    crate::serial::write_line(format_args!(
                        "[S27.5] IPC response len={} decoded={:?} expected={:?}",
                        message.payload().len(),
                        decoded,
                        expected
                    ));
                }
                return decoded == Some(expected);
            }
            if crate::timer::ticks().wrapping_sub(deadline) < u64::MAX / 2 {
                let waiting = crate::ipc::waiter_counts(pid.as_u64(), handle)
                    .map(|counts| counts.0 == 1)
                    .unwrap_or(false);
                crate::serial::write_line(format_args!(
                    "[S27.5] IPC response timeout exited={} pending={} receive_waiting={}",
                    crate::task::process_exited(pid),
                    crate::ipc::message_pending(pid.as_u64()),
                    waiting
                ));
                return false;
            }
            crate::task::yield_now();
        }
    }

    let Ok(verified) = crate::signed_app::verify_package(update_image) else {
        return fail("signed app verification failed");
    };
    if crate::elf::parse(verified.bytes()).is_err() {
        return fail("signed app ELF validation failed");
    }
    let Some(program) = crate::userspace::load_elf_with_argv_verified(
        update_image,
        &["/bin/wovenhat-update", "--service"],
    ) else {
        return fail("daemon load failed");
    };
    let capabilities = crate::capability::CapabilitySet::only(crate::capability::Capability::Ipc)
        .with(crate::capability::Capability::FileRead)
        .with(crate::capability::Capability::FileWrite)
        .with(crate::capability::Capability::StorageIo);
    let Ok((pid, _, _)) =
        crate::task::spawn_user_system_service("woven-update", program, capabilities)
    else {
        return fail("daemon spawn failed");
    };

    let deadline = crate::timer::ticks().wrapping_add(500);
    let published = loop {
        if let Ok(info) = crate::ipc::service_info(b"woven.update") {
            if info.owner == pid.as_u64() && info.client_rights == 1 {
                break true;
            }
        }
        if crate::task::process_exited(pid)
            || crate::timer::ticks().wrapping_sub(deadline) < u64::MAX / 2
        {
            break false;
        }
        crate::task::yield_now();
    };
    if !published {
        let _ = reap(pid);
        return fail("daemon publication failed");
    }
    let Ok(client_handle) = crate::ipc::discover_service(pid.as_u64(), b"woven.update") else {
        let _ = reap(pid);
        return fail("daemon discovery failed");
    };

    let query = Request {
        operation: Operation::Query,
        request_id: 0x27_05_01,
        generation: 0,
        slot: NO_SLOT,
        source: b"",
    };
    let mut request_bytes = [0u8; libwoven::update_ipc::MAX_MESSAGE_BYTES];
    let Ok(query_length) = query.encode(&mut request_bytes) else {
        let _ = reap(pid);
        return fail("query encoding failed");
    };
    if !exchange(
        pid,
        client_handle,
        &request_bytes[..query_length],
        Response {
            request_id: query.request_id,
            status: STATUS_OK,
            // Query packs the active slot in bits 0..=7 and pending slot + 1
            // in bits 8..=15. The direct self-test activates slot 1 before
            // the daemon is spawned, with no pending candidate.
            value: 1,
        },
    ) {
        let _ = reap(pid);
        return fail("query response mismatch");
    }

    // A truncated WUDI frame must be answered without reaching syscall 239.
    if !exchange(
        pid,
        client_handle,
        b"WUDI",
        Response {
            request_id: 0,
            status: STATUS_BAD_REQUEST,
            value: 0,
        },
    ) {
        let _ = reap(pid);
        return fail("malformed response mismatch");
    }
    if !reap(pid) {
        return fail("daemon teardown/reap failed");
    }
    let deadline = crate::timer::ticks().wrapping_add(500);
    while crate::ipc::service_info(b"woven.update").is_ok() {
        if crate::timer::ticks().wrapping_sub(deadline) < u64::MAX / 2 {
            return fail("daemon service cleanup timed out");
        }
        crate::task::yield_now();
    }

    // Restart the signed daemon after an orderly crash/teardown boundary.
    // Reloading the verified image is intentional: a restart must not reuse
    // stale process state or an old endpoint capability.
    let Some(restarted_program) = crate::userspace::load_elf_with_argv_verified(
        update_image,
        &["/bin/wovenhat-update", "--service"],
    ) else {
        return fail("daemon restart load failed");
    };
    let Ok((restarted_pid, _, _)) =
        crate::task::spawn_user_system_service("woven-update", restarted_program, capabilities)
    else {
        return fail("daemon restart spawn failed");
    };
    let restart_deadline = crate::timer::ticks().wrapping_add(500);
    let republished = loop {
        if let Ok(info) = crate::ipc::service_info(b"woven.update") {
            if info.owner == restarted_pid.as_u64() && info.client_rights == 1 {
                break true;
            }
        }
        if crate::task::process_exited(restarted_pid)
            || crate::timer::ticks().wrapping_sub(restart_deadline) < u64::MAX / 2
        {
            break false;
        }
        crate::task::yield_now();
    };
    if !republished {
        let _ = reap(restarted_pid);
        return fail("daemon restart publication failed");
    }
    let Ok(restarted_handle) =
        crate::ipc::discover_service(restarted_pid.as_u64(), b"woven.update")
    else {
        let _ = reap(restarted_pid);
        return fail("daemon restart discovery failed");
    };
    let Ok(restart_query_length) = query.encode(&mut request_bytes) else {
        let _ = reap(restarted_pid);
        return fail("daemon restart query encoding failed");
    };
    if !exchange(
        restarted_pid,
        restarted_handle,
        &request_bytes[..restart_query_length],
        Response {
            request_id: query.request_id,
            status: STATUS_OK,
            value: 1,
        },
    ) {
        let _ = reap(restarted_pid);
        return fail("daemon restart query mismatch");
    }
    if !reap(restarted_pid) {
        return fail("daemon restart teardown/reap failed");
    }
    let restart_cleanup_deadline = crate::timer::ticks().wrapping_add(500);
    while crate::ipc::service_info(b"woven.update").is_ok() {
        if crate::timer::ticks().wrapping_sub(restart_cleanup_deadline) < u64::MAX / 2 {
            return fail("daemon restart service cleanup timed out");
        }
        crate::task::yield_now();
    }
    crate::serial::write_line(format_args!(
        "[S27.5] IPC daemon restart/republication: PASSED"
    ));
    true
}
