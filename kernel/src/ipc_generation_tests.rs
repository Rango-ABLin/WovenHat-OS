//! QEMU-only boundary probes against the real global IPC registry.
//! Only empty counters in freshly registered test namespaces are seeded.
use super::*;

const OWNER: u64 = u64::MAX - 0x810;
const EXHAUSTED: u64 = u64::MAX - 0x811;
const RECEIVER: u64 = u64::MAX - 0x812;
const NAME: &[u8] = b"woven.generation-exhaustion";

fn installation_paths(state: &mut State) -> Option<()> {
    let baseline = state.object_count();
    let stale = state.create_endpoint_object(EXHAUSTED).ok()?;
    state.close_handle(EXHAUSTED, stale).ok()?;
    {
        let space = state.handle_space_mut(EXHAUSTED)?;
        if space.count() != 0 {
            return None;
        }
        space.generations.fill(HandleSpace::MAX_GENERATION);
        space.generations[0] = HandleSpace::MAX_GENERATION - 1;
    }
    let last = state.create_endpoint_object(EXHAUSTED).ok()?;
    if last.generation() != HandleSpace::MAX_GENERATION
        || state.handle_info(EXHAUSTED, last).is_err()
    {
        return None;
    }
    state.close_handle(EXHAUSTED, last).ok()?;
    for _ in 0..2 {
        if state.create_endpoint_object(EXHAUSTED) != Err(Error::HandleTableFull)
            || state.handle_info(EXHAUSTED, stale) != Err(Error::InvalidHandle)
            || state.handle_info(EXHAUSTED, last) != Err(Error::InvalidHandle)
            || state.object_count() != baseline
        {
            return None;
        }
    }

    let source = state.create_endpoint_object(OWNER).ok()?;
    let original = state.handle_space(OWNER)?.lookup_raw(source).ok()?;
    if state.grant_handle(OWNER, source, EXHAUSTED, HandleRights::SEND)
        != Err(Error::HandleTableFull)
        || state.object(original.object)?.references != 1
        || state.handle_info(OWNER, source).ok()?.rights != HandleRights::ENDPOINT_OWNER
    {
        return None;
    }
    let lineage = state.object(original.object)?.delegation_lineage?;
    if !wovenguard::lineage_authorizes(lineage, Capability::Ipc) {
        return None;
    }
    state
        .publish_service(OWNER, NAME, source, HandleRights::SEND)
        .ok()?;
    let services = state.service_count();
    for _ in 0..2 {
        if state.discover_service(EXHAUSTED, NAME) != Err(Error::HandleTableFull)
            || state.object(original.object)?.references != 2
            || state.service_count() != services
        {
            return None;
        }
    }

    // Keep a live receive handle and one ordinary reusable filler. Every other
    // slot is empty and permanently exhausted; no counter is ever reset.
    let channel = state.create_endpoint_object(RECEIVER).ok()?;
    let filler = state.create_endpoint_object(RECEIVER).ok()?;
    {
        let space = state.handle_space_mut(RECEIVER)?;
        for (index, entry) in space.entries.iter().enumerate() {
            if entry.is_none() {
                space.generations[index] = HandleSpace::MAX_GENERATION;
            }
        }
    }
    let sender = state
        .grant_handle(RECEIVER, channel, OWNER, HandleRights::SEND)
        .ok()?;
    let rights = HandleRights::SEND | HandleRights::INSPECT;
    state
        .send_handle_with_transfer(OWNER, sender, source, rights, b"retained-escrow")
        .ok()?;
    let channel_ref = state
        .handle_space(RECEIVER)?
        .lookup_raw(channel)
        .ok()?
        .object;
    let send_waiter = Waiter {
        owner: OWNER,
        handle: sender,
        task: TaskId::from_u64(0x810),
    };
    let receive_waiter = Waiter {
        owner: RECEIVER,
        handle: channel,
        task: TaskId::from_u64(0x811),
    };
    {
        let object = state.object_mut(channel_ref)?;
        push_waiter(&mut object.send_waiters, send_waiter).ok()?;
        push_waiter(&mut object.receive_waiters, receive_waiter).ok()?;
    }
    let head = state.object(channel_ref)?.head().ok()?;
    if state.receive_handle(RECEIVER, channel) != Err(Error::HandleTableFull) {
        return None;
    }
    {
        let object = state.object(channel_ref)?;
        let after = object.head().ok()?;
        if object.length != 1
            || after.message != head.message
            || after.transfer != head.transfer
            || object.send_waiters[0] != Some(send_waiter)
            || object.receive_waiters[0] != Some(receive_waiter)
            || state.object(original.object)?.references != 3
        {
            return None;
        }
    }
    state.close_handle(RECEIVER, filler).ok()?;
    let (message, wake) = state.receive_handle(RECEIVER, channel).ok()?;
    let received = message.transferred_handle()?;
    let installed = state.handle_space(RECEIVER)?.lookup_raw(received).ok()?;
    if received.slot() != filler.slot()
        || received == filler
        || message.payload() != b"retained-escrow"
        || message.sender != OWNER
        || installed.object != original.object
        || installed.rights != rights
        || installed.lineage != Some(lineage)
        || installed.service_class != original.service_class
        || wake != Some(send_waiter.task)
        || state.object(original.object)?.references != 3
        || state.receive_handle(RECEIVER, channel) != Err(Error::QueueEmpty)
        || state.handle_info(RECEIVER, filler) != Err(Error::InvalidHandle)
    {
        return None;
    }
    // Close the now-empty channel while its receive waiter is still pending.
    // A later send would legitimately consume that waiter instead.
    let wakes = state.close_handle(RECEIVER, channel).ok()?;
    if !wakes.contains(&Some(receive_waiter.task)) || state.object(channel_ref)?.references != 1 {
        return None;
    }
    let old_channel = channel;
    let old_channel_ref = channel_ref;
    let channel = state.create_endpoint_object(RECEIVER).ok()?;
    if channel == old_channel
        || channel.slot() != old_channel.slot()
        || state.handle_info(RECEIVER, old_channel) != Err(Error::InvalidHandle)
    {
        return None;
    }
    let channel_ref = state
        .handle_space(RECEIVER)?
        .lookup_raw(channel)
        .ok()?
        .object;
    let sender = state
        .grant_handle(RECEIVER, channel, OWNER, HandleRights::SEND)
        .ok()?;
    let send_waiter = Waiter {
        handle: sender,
        ..send_waiter
    };
    // Queue another owning escrow and a waiter so publisher-exit and abandoned
    // queue teardown assertions are not satisfied vacuously by empty tables.
    state
        .send_handle_with_transfer(OWNER, sender, source, rights, b"abandoned-after-exhaustion")
        .ok()?;
    push_waiter(
        &mut state.object_mut(channel_ref)?.send_waiters,
        send_waiter,
    )
    .ok()?;
    if state.object(original.object)?.references != 4 {
        crate::serial::write_line(format_args!(
            "[IPC EXHAUST] second escrow refs={:?}",
            state
                .object(original.object)
                .map(|object| object.references)
        ));
        return None;
    }
    // The receiver handle and queue escrow outlive the publisher independently.
    // Publisher exit removes discovery and its actual pending waiter.
    state.unregister(OWNER).ok()?;
    if state.discover_service(EXHAUSTED, NAME) != Err(Error::ServiceNotFound)
        || state.object(original.object)?.references != 2
        || state.object(old_channel_ref).is_some()
        || state
            .object(channel_ref)?
            .send_waiters
            .iter()
            .flatten()
            .any(|w| w.owner == OWNER)
    {
        return None;
    }
    let wakes = state.close_handle(RECEIVER, channel).ok()?;
    if wakes.iter().any(Option::is_some)
        || state.object(channel_ref).is_some()
        || state.object(original.object)?.references != 1
    {
        return None;
    }
    // Leave received live: unregister must drop the final object/lineage ref.
    state.unregister(RECEIVER).ok()?;
    if state.object(original.object).is_some() || state.object_count() != baseline {
        return None;
    }
    Some(())
}

pub(super) fn run() -> bool {
    let baseline_frames = crate::memory::stats().allocated_frames;
    let baseline_objects = object_count();
    let baseline_services = service_count();
    let baseline_lineages = wovenguard::lineage_count();
    let owners = [OWNER, EXHAUSTED, RECEIVER];
    let mut registered = [false; 3];
    let passed = (|| {
        for (index, owner) in owners.iter().copied().enumerate() {
            register(owner).ok()?;
            registered[index] = true;
        }
        installation_paths(&mut STATE.lock())?;
        // Exercise the public allocator's frame-ownership rollback as well as
        // private installation. Never allocate frames or schedule under STATE.
        for _ in 0..2 {
            if create_shared_memory(EXHAUSTED, 1) != Err(Error::HandleTableFull)
                || crate::memory::stats().allocated_frames != baseline_frames
                || shared_memory_count() != 0
            {
                return None;
            }
        }
        Some(())
    })()
    .is_some();
    // Clean only namespaces this probe successfully registered, even if an
    // assertion failed. Already-unregistered owners may return NoEndpoint.
    for (index, owner) in owners.into_iter().enumerate() {
        if registered[index] {
            let _ = unregister(owner);
        }
    }
    let clean = object_count() == baseline_objects
        && service_count() == baseline_services
        && wovenguard::lineage_count() == baseline_lineages
        && crate::memory::stats().allocated_frames == baseline_frames;
    crate::serial::write_line(format_args!(
        "[IPC EXHAUST] body={} clean={} objects={}/{} services={}/{} lineages={}/{} frames={}/{}",
        passed,
        clean,
        object_count(),
        baseline_objects,
        service_count(),
        baseline_services,
        wovenguard::lineage_count(),
        baseline_lineages,
        crate::memory::stats().allocated_frames,
        baseline_frames
    ));
    crate::serial::write_line(format_args!(
        "[S8.1] exhausted-generation installation/rollback: {}",
        if passed && clean { "PASSED" } else { "FAILED" }
    ));
    passed && clean
}
