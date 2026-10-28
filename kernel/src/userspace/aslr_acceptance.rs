use super::*;

fn guard_fixture(mmap_base: u64, guard: u64) -> Option<alloc::vec::Vec<u8>> {
    // Architecture-specific user instructions exercise the ordinary Mmap ABI.
    // Every allocation must return its independently expected slot address;
    // an allocation failure cannot masquerade as successful guard isolation.
    let mut code = alloc::vec::Vec::new();
    let mut failures = alloc::vec::Vec::new();
    for slot in 0..16_u64 {
        code.extend_from_slice(&[0xb8, 8, 0, 0, 0]); // Mmap
        code.extend_from_slice(&[0xbf, 0, 0, 1, 0]); // 65536 bytes
        code.extend_from_slice(&[0xbe, 1, 0, 0, 0, 0xcd, 0x80]); // writable
        code.extend_from_slice(&[0x48, 0xbb]);
        code.extend_from_slice(&(mmap_base + slot * 65536).to_le_bytes());
        code.extend_from_slice(&[0x48, 0x39, 0xd8, 0x0f, 0x85]); // cmp; jne failure
        failures.push(code.len());
        code.extend_from_slice(&[0; 4]);
    }
    code.extend_from_slice(&[0x48, 0xb8]);
    code.extend_from_slice(&guard.to_le_bytes());
    code.extend_from_slice(&[0xc6, 0x00, 42]); // Must fault, not write the guard.
    code.extend_from_slice(&[0xbf, 99, 0, 0, 0, 0xb8, 3, 0, 0, 0, 0xcd, 0x80, 0x0f, 0x0b]);
    let failure = code.len();
    code.extend_from_slice(&[0xbf, 98, 0, 0, 0, 0xb8, 3, 0, 0, 0, 0xcd, 0x80, 0x0f, 0x0b]);
    for offset in failures {
        let relative = i32::try_from(failure.checked_sub(offset + 4)?).ok()?;
        code[offset..offset + 4].copy_from_slice(&relative.to_le_bytes());
    }
    build_stub_elf(&code)
}

pub(super) fn run() -> bool {
    // A supported production pair: stack draw 16, mmap draw 0. The final
    // slot ends exactly at the old stack base, without touching mapped stack.
    // The complete 16-slot mmap arena ends at 0x1e_0000.  The guard must be
    // above that arena, not inside its final 64 KiB slot.
    let stack_base = USER_REGION_START + 0x1e_1000;
    let guard = USER_REGION_START + 0x1e_0000;
    let mmap_base = USER_REGION_START + 0xe_0000;
    let Some(bytes) = guard_fixture(mmap_base, guard) else {
        return false;
    };
    crate::heap::maintain_capacity();
    let frames = crate::memory::stats().allocated_frames;
    let processes = crate::task::process_count();
    let endpoints = crate::ipc::endpoint_count();
    let Some(program) =
        linker::load_with_geometry(&bytes, &["aslr-guard"], 0, stack_base, mmap_base)
    else {
        return false;
    };
    if program.stack.guard_base != guard
        || !paging::user_range_is_unmapped_in(
            program.address_space.paging,
            guard,
            UserStack::GUARD_SIZE,
        )
    {
        let _ = destroy(program.address_space);
        return false;
    }
    let Ok((pid, _)) = crate::task::spawn_user_process("aslr-guard", program) else {
        return false;
    };
    let start = crate::timer::ticks();
    while !crate::task::process_exited(pid) {
        if crate::timer::ticks().wrapping_sub(start) > 500 {
            return false;
        }
        crate::task::yield_now();
    }
    let status = crate::task::wait_process(pid.as_u64());
    let after = crate::memory::stats().allocated_frames;
    let passed = status == Ok(-11)
        && frames == after
        && crate::task::process_count() == processes
        && crate::ipc::endpoint_count() == endpoints
        && crate::thread::reap_owner(pid.as_u64()) == 0;
    crate::serial::write_line(format_args!(
        "[S11.ASLR] forced stack=16 mmap=0 guard={guard:#x} status={:?} frames={frames}/{after}: {}",
        status.as_ref().ok(),
        if passed { "PASSED" } else { "FAILED" },
    ));
    passed
}
