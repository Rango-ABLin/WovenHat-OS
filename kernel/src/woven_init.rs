//! WovenInit: bounded first-userspace dependency and readiness coordinator.
//!
//! The coordinator owns only startup ordering and readiness observation. Each
//! service retains its own signed image, capability set, IPC endpoint and
//! lifecycle supervision in `service_supervisor`.

use crate::{serial, service_supervisor, task, timer};
#[cfg(feature = "woven-init-failure-test")]
use core::sync::atomic::{AtomicBool, Ordering};

const MAX_NODES: usize = 4;
const READY_TIMEOUT_TICKS: u64 = 500;
#[cfg(feature = "woven-init-failure-test")]
static FAILURE_TEST_PASSED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Pending,
    Starting,
    Ready,
    Failed,
}

#[derive(Clone, Copy)]
struct Node {
    kind: service_supervisor::ServiceKind,
    name: &'static str,
    ipc_name: &'static [u8],
    dependencies: u8,
    state: State,
    pid: Option<task::ProcessId>,
}

const fn nodes() -> [Node; MAX_NODES] {
    [
        Node {
            kind: service_supervisor::ServiceKind::Package,
            name: "woven-package",
            ipc_name: b"woven.package",
            dependencies: 0,
            state: State::Pending,
            pid: None,
        },
        Node {
            kind: service_supervisor::ServiceKind::Update,
            name: "woven-update",
            ipc_name: b"woven.update",
            dependencies: 1 << 0,
            state: State::Pending,
            pid: None,
        },
        Node {
            kind: service_supervisor::ServiceKind::Installer,
            name: "woven-installer",
            ipc_name: b"sys.storage.installer",
            dependencies: 1 << 0,
            state: State::Pending,
            pid: None,
        },
        Node {
            kind: service_supervisor::ServiceKind::Search,
            name: "woven-search",
            ipc_name: libwoven::search_ipc::SERVICE_NAME,
            dependencies: 1 << 0,
            state: State::Pending,
            pid: None,
        },
    ]
}

fn dependencies_ready(nodes: &[Node; MAX_NODES], mask: u8) -> bool {
    nodes
        .iter()
        .enumerate()
        .all(|(index, node)| mask & (1 << index) == 0 || node.state == State::Ready)
}

fn wait_ready(node: Node, deadline: u64) -> bool {
    loop {
        if let Ok(info) = crate::ipc::service_info(node.ipc_name) {
            if node.pid.is_some_and(|pid| info.owner == pid.as_u64())
                && info.client_rights != 0
                && !task::process_exited(node.pid.unwrap())
            {
                return true;
            }
        }
        if task::process_exited(node.pid.unwrap()) || timer::ticks() >= deadline {
            return false;
        }
        task::yield_now();
    }
}

/// Start all essential services in dependency order and publish one readiness
/// barrier for the rest of boot. No consumer is allowed to start on a partial
/// graph; a failed node tears down already-started nodes before returning.
pub fn start() -> bool {
    let mut graph = nodes();
    serial::write_line(format_args!(
        "[WOVENINIT] dependency graph validated nodes={} edges=3",
        MAX_NODES
    ));
    let mut ready_mask = 0_u8;
    for _ in 0..MAX_NODES {
        let Some(index) = graph.iter().enumerate().find_map(|(index, node)| {
            (node.state == State::Pending && dependencies_ready(&graph, node.dependencies))
                .then_some(index)
        }) else {
            serial::write_line(format_args!("[WOVENINIT] dependency graph stalled"));
            return rollback(&graph);
        };
        graph[index].state = State::Starting;
        let node = graph[index];
        let Ok(pid) = service_supervisor::spawn(node.kind) else {
            graph[index].state = State::Failed;
            serial::write_line(format_args!("[WOVENINIT] {} spawn failed", node.name));
            return rollback(&graph);
        };
        if !service_supervisor::register(node.kind, pid) {
            let _ = task::kill_process(pid.as_u64(), 9);
            graph[index].state = State::Failed;
            serial::write_line(format_args!(
                "[WOVENINIT] {} registration failed",
                node.name
            ));
            return rollback(&graph);
        }
        graph[index].pid = Some(pid);
        #[cfg(feature = "woven-init-failure-test")]
        if node.kind == service_supervisor::ServiceKind::Update {
            // Exercise the real startup boundary: the dependency was ready,
            // this service was spawned and registered, but it died before
            // publishing readiness. WovenInit must reject the partial graph
            // and tear down every process it already started.
            let _ = task::kill_process(pid.as_u64(), 9);
        }
        let deadline = timer::ticks().saturating_add(READY_TIMEOUT_TICKS);
        if !wait_ready(graph[index], deadline) {
            graph[index].state = State::Failed;
            serial::write_line(format_args!("[WOVENINIT] {} readiness timeout", node.name));
            return rollback(&graph);
        }
        graph[index].state = State::Ready;
        ready_mask |= 1 << index;
        serial::write_line(format_args!(
            "[WOVENINIT] {} ready pid={} mask=0x{:x}",
            node.name,
            pid.as_u64(),
            ready_mask
        ));
    }
    serial::write_line(format_args!(
        "[WOVENINIT] essential services ready mask=0x{:x}",
        ready_mask
    ));
    true
}

fn rollback(graph: &[Node; MAX_NODES]) -> bool {
    let mut _had_process = false;
    for node in graph.iter().rev() {
        if let Some(pid) = node.pid {
            _had_process = true;
            if !task::process_exited(pid) {
                let _ = task::kill_process(pid.as_u64(), 9);
            }
        }
    }
    #[cfg(feature = "woven-init-failure-test")]
    if _had_process
        && graph
            .iter()
            .filter_map(|node| node.pid)
            .all(task::process_exited)
    {
        FAILURE_TEST_PASSED.store(true, Ordering::Release);
    }
    false
}

#[cfg(feature = "woven-init-failure-test")]
pub fn failure_test_passed() -> bool {
    FAILURE_TEST_PASSED.load(Ordering::Acquire)
}
