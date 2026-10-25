//! Isolated native C++ ABI acceptance, without production privilege changes.
use crate::{
    capability::{Capability, CapabilitySet},
    memory, serial, task, timer, userspace,
};
const IMAGE: &[u8] = include_bytes!(env!("WOVENHAT_WEB_CXX_PROBE_IMAGE"));
pub fn run() -> bool {
    let frames = memory::stats().allocated_frames;
    let mappings = task::anonymous_mapping_count();
    let processes = task::process_count();
    let Some(program) = userspace::load_elf_with_argv(IMAGE, &["wovenweb-cxx-probe"]) else {
        return false;
    };
    let ceiling = CapabilitySet::only(Capability::Console);
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
