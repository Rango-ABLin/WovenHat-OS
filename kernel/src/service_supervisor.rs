//! Bounded supervision for production system services.
//!
//! The supervisor owns restart policy and PID generations only. Authority
//! remains in each signed userspace image and the kernel IPC boundary.

use crate::{capability, irq_lock::IrqMutex, task, timer, userspace};

const MAX_SERVICES: usize = 3;
const MAX_RESTARTS: u8 = 3;
const BASE_BACKOFF_TICKS: u64 = 100;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ServiceKind {
    Package,
    Update,
    Installer,
}

#[derive(Clone, Copy)]
struct Entry {
    kind: ServiceKind,
    pid: Option<task::ProcessId>,
    restart_count: u8,
    next_attempt: u64,
}

struct State {
    entries: [Option<Entry>; MAX_SERVICES],
}

impl State {
    const fn new() -> Self {
        Self {
            entries: [None; MAX_SERVICES],
        }
    }
}

static STATE: IrqMutex<State> = IrqMutex::with_rank(State::new(), 10);

pub fn register(kind: ServiceKind, pid: task::ProcessId) -> bool {
    let mut state = STATE.lock();
    if state
        .entries
        .iter()
        .flatten()
        .any(|entry| entry.kind == kind)
    {
        return false;
    }
    let Some(slot) = state.entries.iter_mut().find(|entry| entry.is_none()) else {
        return false;
    };
    *slot = Some(Entry {
        kind,
        pid: Some(pid),
        restart_count: 0,
        next_attempt: 0,
    });
    true
}

/// Poll once from the kernel event loop. No process operation occurs while
/// the supervisor lock is held, and stale PID generations cannot be replaced.
pub fn poll() {
    let snapshot = {
        let state = STATE.lock();
        state.entries
    };
    let now = timer::ticks();
    for (index, entry) in snapshot.into_iter().enumerate() {
        let Some(entry) = entry else { continue };
        if now < entry.next_attempt {
            continue;
        }
        if let Some(pid) = entry.pid {
            if !task::process_exited(pid) {
                continue;
            }
            let _ = task::wait_process(pid.as_u64());
        }
        if entry.restart_count >= MAX_RESTARTS {
            let mut state = STATE.lock();
            if state
                .entries
                .get(index)
                .copied()
                .flatten()
                .is_some_and(|current| current.kind == entry.kind && current.pid == entry.pid)
            {
                state.entries[index] = None;
            }
            crate::serial::write_line(format_args!(
                "[SUPERVISOR] {} restart budget exhausted; service disabled",
                name(entry.kind)
            ));
            continue;
        }
        let backoff = BASE_BACKOFF_TICKS.saturating_mul(1_u64 << u32::from(entry.restart_count));
        let next_attempt = now.saturating_add(backoff);
        let pending = Entry {
            kind: entry.kind,
            pid: None,
            restart_count: entry.restart_count,
            next_attempt,
        };
        update(index, entry, None, next_attempt, entry.restart_count);
        let Ok(pid) = spawn(entry.kind) else {
            update(index, pending, None, next_attempt, entry.restart_count + 1);
            crate::serial::write_line(format_args!(
                "[SUPERVISOR] {} restart attempt failed; retry scheduled",
                name(entry.kind)
            ));
            continue;
        };
        let mut state = STATE.lock();
        if state
            .entries
            .get(index)
            .copied()
            .flatten()
            .is_some_and(|current| current.kind == entry.kind && current.pid.is_none())
        {
            state.entries[index] = Some(Entry {
                kind: entry.kind,
                pid: Some(pid),
                restart_count: entry.restart_count + 1,
                next_attempt,
            });
            crate::serial::write_line(format_args!(
                "[SUPERVISOR] {} restarted as pid {}",
                name(entry.kind),
                pid.as_u64()
            ));
        } else {
            let _ = task::kill_process(pid.as_u64(), 9);
        }
    }
}

fn update(
    index: usize,
    old: Entry,
    pid: Option<task::ProcessId>,
    next_attempt: u64,
    restart_count: u8,
) {
    let mut state = STATE.lock();
    if state
        .entries
        .get(index)
        .copied()
        .flatten()
        .is_some_and(|current| current.kind == old.kind && current.pid == old.pid)
    {
        state.entries[index] = Some(Entry {
            kind: old.kind,
            pid,
            restart_count,
            next_attempt,
        });
    }
}

fn capabilities() -> capability::CapabilitySet {
    capability::CapabilitySet::only(capability::Capability::Ipc)
        .with(capability::Capability::FileRead)
        .with(capability::Capability::FileWrite)
        .with(capability::Capability::StorageIo)
}

fn spawn(kind: ServiceKind) -> Result<task::ProcessId, task::ProcessError> {
    let capabilities = capabilities();
    match kind {
        ServiceKind::Package => spawn_one(
            "woven-package",
            crate::PACKAGE_SERVICE_IMAGE,
            &["/bin/woven-package", "--service"],
            capabilities,
        ),
        ServiceKind::Update => spawn_one(
            "woven-update",
            crate::SYSTEM_APP_UPDATE,
            &["/bin/wovenhat-update", "--service"],
            capabilities,
        ),
        ServiceKind::Installer => spawn_one(
            "woven-installer",
            crate::SYSTEM_APP_INSTALLER,
            &["/bin/wovenhat-installer", "--service"],
            capabilities,
        ),
    }
}

fn spawn_one(
    name: &'static str,
    image: &'static [u8],
    argv: &'static [&'static str],
    capabilities: capability::CapabilitySet,
) -> Result<task::ProcessId, task::ProcessError> {
    let program =
        userspace::load_elf_with_argv_verified(image, argv).ok_or(task::ProcessError::Full)?;
    let (pid, _, _) = task::spawn_user_system_service(name, program, capabilities)?;
    Ok(pid)
}

fn name(kind: ServiceKind) -> &'static str {
    match kind {
        ServiceKind::Package => "woven-package",
        ServiceKind::Update => "woven-update",
        ServiceKind::Installer => "woven-installer",
    }
}
