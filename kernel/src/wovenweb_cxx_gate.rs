//! Isolated native C++ ABI acceptance, without production privilege changes.
use crate::{
    capability::{Capability, CapabilitySet},
    memory, serial, task, timer, userspace,
};
const IMAGE: &[u8] = include_bytes!(env!("WOVENHAT_WEB_CXX_PROBE_IMAGE"));
const C_ABI_IMAGE: &[u8] = include_bytes!(env!("WOVENHAT_RUNTIME_C_ABI_ELF"));
pub fn run() -> bool {
    let frames = memory::stats().allocated_frames;
    let mappings = task::anonymous_mapping_count();
    let processes = task::process_count();
    let Some(c_program) = userspace::load_elf_with_argv(C_ABI_IMAGE, &["runtime-c-abi"]) else {
        return false;
    };
    let ceiling = CapabilitySet::only(Capability::Console);
    let Ok((c_pid, _)) = task::spawn_user_process_with_sandbox(
        "runtime-c-abi",
        c_program,
        crate::wovenguard::SandboxProfile::new(0x575a, ceiling),
    ) else {
        return false;
    };
    let c_start = timer::ticks();
    while !task::process_exited(c_pid) {
        if timer::ticks().wrapping_sub(c_start) > 1500 {
            return false;
        }
        task::yield_now();
    }
    let c_status = task::wait_process(c_pid.as_u64());
    serial::write_line(format_args!(
        "[WOVENWEB-C] exit={:?} mappings={}/{} frames={}/{} processes={}/{}",
        c_status.ok(),
        task::anonymous_mapping_count(),
        mappings,
        memory::stats().allocated_frames,
        frames,
        task::process_count(),
        processes
    ));
    if c_status != Ok(42)
        || task::anonymous_mapping_count() != mappings
        || memory::stats().allocated_frames != frames
        || task::process_count() != processes
    {
        return false;
    }
    serial::write_line(format_args!(
        "[WOVENWEB-C] freestanding C _start + exit ABI: PASSED"
    ));
    let Some(program) = userspace::load_elf_with_argv(IMAGE, &["wovenweb-cxx-probe"]) else {
        return false;
    };
    let Ok((pid, _)) = task::spawn_user_process_with_sandbox(
        "wovenweb-cxx-probe",
        program,
        crate::wovenguard::SandboxProfile::new(0x5759, ceiling),
    ) else {
        return false;
    };
    let start = timer::ticks();
    while !task::process_exited(pid) {
        if timer::ticks().wrapping_sub(start) > 1500 {
            return false;
        }
        // The bootstrap task cannot park on a process event. Yield its unused
        // quantum, as the other Ring-3 acceptance gates do; HLT would retain a
        // full bootstrap quantum after each user yield and starve the fixture.
        task::yield_now();
    }
    let status = task::wait_process(pid.as_u64());
    serial::write_line(format_args!(
        "[WOVENWEB-CXX] exit={:?} mappings={}/{} frames={}/{} processes={}/{}",
        status.ok(),
        task::anonymous_mapping_count(),
        mappings,
        memory::stats().allocated_frames,
        frames,
        task::process_count(),
        processes
    ));
    status == Ok(0)
        && task::anonymous_mapping_count() == mappings
        && memory::stats().allocated_frames == frames
        && task::process_count() == processes
}
