//! Feature-isolated native SDK acceptance, not a production browser launch.
use crate::{
    async_network, async_op,
    capability::{Capability, CapabilitySet},
    network, serial, task, timer, userspace,
};
const IMAGE: &[u8] = include_bytes!(env!("WOVENHAT_WEB_NET_PROBE_IMAGE"));
fn child(denied: bool) -> bool {
    let argv: &[&str] = if denied {
        &["wovenweb-net-probe", "--denied"]
    } else {
        &["wovenweb-net-probe"]
    };
    let Some(program) = userspace::load_elf_with_argv(IMAGE, argv) else {
        return false;
    };
    let capabilities = if denied {
        CapabilitySet::only(Capability::Console)
    } else {
        CapabilitySet::only(Capability::Console).with(Capability::NetworkIo)
    };
    let Ok((pid, _)) = task::spawn_user_process_with_sandbox(
        "wovenweb-net-probe",
        program,
        crate::wovenguard::SandboxProfile::new(0x5757, capabilities),
    ) else {
        return false;
    };
    let start = timer::ticks();
    while !task::process_exited(pid) {
        network::poll();
        if timer::ticks().wrapping_sub(start) > 1500 {
            return false;
        }
        // No IRQ lock is held. The timer/NIC interrupt wakes this hardware pump.
        x86_64::instructions::hlt();
    }
    let status = task::wait_process(pid.as_u64());
    serial::write_line(format_args!(
        "[WOVENWEB] native probe denied={} exit={:?}",
        denied,
        status.ok()
    ));
    status == Ok(0)
}
pub fn run() -> bool {
    if network::init().is_err() || !async_network::start_worker() {
        return false;
    }
    let sockets = network::stats().user_sockets;
    let active = async_op::stats().active;
    let processes = task::process_count();
    let ports = crate::completion_port::active_count();
    if !child(true) || !child(false) {
        return false;
    }
    let start = timer::ticks();
    while async_network::stats().active != 0 || network::stats().user_sockets != sockets {
        network::poll();
        if timer::ticks().wrapping_sub(start) > 200 {
            return false;
        }
        x86_64::instructions::hlt();
    }
    let after = async_op::stats();
    serial::write_line(format_args!(
        "[WOVENWEB] cleanup async={}/{} sockets={}/{} processes={}/{} ports={}/{}",
        after.active,
        active,
        network::stats().user_sockets,
        sockets,
        task::process_count(),
        processes,
        crate::completion_port::active_count(),
        ports
    ));
    after.active == active
        && task::process_count() == processes
        && crate::completion_port::active_count() == ports
}
