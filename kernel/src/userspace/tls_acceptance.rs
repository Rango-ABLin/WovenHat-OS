use super::*;

fn edge_fixture(
    offset: usize,
    memory: u64,
    alignment: u64,
    span: i32,
) -> Option<alloc::vec::Vec<u8>> {
    // CPU-specific FS:0/local-exec probe with an independently specified
    // linked displacement. Do not derive the expected displacement from Layout.
    let mut payload = alloc::vec![0; offset.checked_add(8)?];
    let code = [
        0x64, 0x48, 0x8b, 0x04, 0x25, 0, 0, 0, 0, 0x48, 0x8b, 0xb8, 0, 0, 0, 0, 0xb8, 3, 0, 0, 0,
        0xcd, 0x80, 0x0f, 0x0b,
    ];
    payload[..code.len()].copy_from_slice(&code);
    payload[12..16].copy_from_slice(&(-span).to_le_bytes());
    payload[offset..offset + 8].copy_from_slice(&42_u64.to_le_bytes());
    let mut bytes = build_stub_elf(&payload)?;
    write_u16(&mut bytes, 56, 2)?;
    write_u32(&mut bytes, 120, 7)?;
    write_u64(&mut bytes, 128, 4096 + offset as u64)?;
    write_u64(&mut bytes, 136, USER_REGION_START + offset as u64)?;
    write_u64(&mut bytes, 152, 8)?;
    write_u64(&mut bytes, 160, memory)?;
    write_u64(&mut bytes, 168, alignment)?;
    Some(bytes)
}

fn protected_layout_and_fork(program: &UserProgram) -> bool {
    let Some(tls) = program.address_space.tls else {
        return false;
    };
    let Some((template, template_size)) = tls.template else {
        return false;
    };
    let pointer = tls.start + tls.layout.thread_pointer_offset as u64;
    let mut word = [0; 8];
    let protected = paging::read_user_bytes_in(program.address_space.paging, pointer, &mut word)
        .is_ok()
        && u64::from_le_bytes(word) == pointer
        && program.tls_base == pointer
        && paging::user_range_has_protection_in(
            program.address_space.paging,
            tls.start,
            tls.layout.mapping_size,
            true,
            false,
        )
        && paging::user_range_has_protection_in(
            program.address_space.paging,
            template,
            template_size,
            false,
            false,
        )
        && program
            .address_space
            .is_logically_writable(tls.start, &[None; MAX_ANONYMOUS_MAPPINGS])
        && !program
            .address_space
            .is_logically_writable(template, &[None; MAX_ANONYMOUS_MAPPINGS])
        && paging::user_range_is_unmapped_in(
            program.address_space.paging,
            program.stack.guard_base,
            UserStack::GUARD_SIZE,
        );
    if !protected {
        return false;
    }
    let Some(clone) = clone_address_space(program.address_space, &[None; MAX_ANONYMOUS_MAPPINGS])
    else {
        return false;
    };
    let copied = paging::read_user_bytes_in(clone.paging, pointer, &mut word).is_ok()
        && u64::from_le_bytes(word) == pointer
        && paging::user_range_has_protection_in(
            clone.paging,
            template,
            template_size,
            false,
            false,
        )
        && !clone.is_logically_writable(template, &[None; MAX_ANONYMOUS_MAPPINGS]);
    let destroyed = destroy(clone);
    copied && destroyed
}

pub(super) fn run() -> bool {
    let Some(nonzero) = edge_fixture(72, 136, 64, 184) else {
        return false;
    };
    let Some(page_edge) = edge_fixture(4096, 4096, 4096, 4096) else {
        return false;
    };
    for (name, bytes) in [
        (
            "static compiler TLS",
            include_bytes!(env!("WOVENHAT_TLS_STATIC_ELF")).as_slice(),
        ),
        (
            "fresh child TLS",
            include_bytes!(env!("WOVENHAT_TLS_THREADS_ELF")).as_slice(),
        ),
        ("nonzero congruence TLS", nonzero.as_slice()),
        ("page-edge TCB", page_edge.as_slice()),
    ] {
        crate::heap::maintain_capacity();
        let frames = crate::memory::stats().allocated_frames;
        let processes = crate::task::process_count();
        let endpoints = crate::ipc::endpoint_count();
        let Some(program) = load_elf(bytes) else {
            crate::serial::write_line(format_args!("[S11.TLS] {name}: FAILED load"));
            return false;
        };
        if !protected_layout_and_fork(&program) {
            let _ = destroy(program.address_space);
            return false;
        }
        let Ok((pid, _)) = crate::task::spawn_user_process("compiler-tls", program) else {
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
        let passed = status == Ok(42)
            && after == frames
            && crate::task::process_count() == processes
            && crate::ipc::endpoint_count() == endpoints
            && crate::thread::reap_owner(pid.as_u64()) == 0;
        crate::serial::write_line(format_args!(
            "[S11.TLS] {name}: {} status={:?} frames={frames}/{after}",
            if passed { "PASSED" } else { "FAILED" },
            status.as_ref().ok()
        ));
        if !passed {
            return false;
        }
        let oversized = alloc::string::String::from_utf8(alloc::vec![b'x'; 256]).unwrap();
        let rollback_frames = crate::memory::stats().allocated_frames;
        let rejected = match load_elf_with_argv(bytes, &[&oversized]) {
            Some(program) => {
                let _ = destroy(program.address_space);
                false
            }
            None => true,
        };
        if !rejected || crate::memory::stats().allocated_frames != rollback_frames {
            return false;
        }
    }
    crate::serial::write_line(format_args!(
        "[S11.TLS] template/TCB rollback and fork protection: PASSED"
    ));
    true
}
