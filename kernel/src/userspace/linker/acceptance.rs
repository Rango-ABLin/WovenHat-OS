use super::*;

// RFC 8032 public test vector. This signing fixture is compiled only into the
// Stage 11.5 QEMU test image, never into production kernels.
const SEED: [u8; 32] = [
    0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c, 0xc4,
    0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae, 0x7f, 0x60,
];

fn fixture(kind: usize) -> Option<Vec<u8>> {
    let base = if kind == 0 || kind == 3 {
        USER_REGION_START
    } else {
        0
    };
    let mut elf = alloc::vec![0; 0x1400];
    elf[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    write_u16(&mut elf, 16, 3)?;
    write_u16(&mut elf, 18, 0x3e)?;
    write_u32(&mut elf, 20, 1)?;
    write_u64(&mut elf, 24, base)?;
    write_u64(&mut elf, 32, 64)?;
    write_u16(&mut elf, 52, 64)?;
    write_u16(&mut elf, 54, 56)?;
    write_u16(&mut elf, 56, 4)?;
    for (offset, typ, flags, file_offset, address, file_size, memory_size) in [
        (64, 1, 5, 0x1000, base, 0x400, 0x1000),
        (120, 1, 6, 0, base + 0x1000, 0, 0x1000),
        (176, 2, 4, 0x1100, base + 0x100, 0xc0, 0xc0),
        (232, 0x6474_e552, 4, 0, base + 0x1000, 0, 0x1000),
    ] {
        write_u32(&mut elf, offset, typ)?;
        write_u32(&mut elf, offset + 4, flags)?;
        write_u64(&mut elf, offset + 8, file_offset)?;
        write_u64(&mut elf, offset + 16, address)?;
        write_u64(&mut elf, offset + 32, file_size)?;
        write_u64(&mut elf, offset + 40, memory_size)?;
        write_u64(&mut elf, offset + 48, 0x1000)?;
    }
    let mut tags = alloc::vec![
        (4, base + 0x280),
        (5, base + 0x300),
        (10, 25),
        (6, base + 0x200),
        (11, 24),
    ];
    if kind == 3 {
        tags.extend([(17, base + 0x380), (18, 16), (19, 16)]);
    } else {
        tags.extend([(7, base + 0x380), (8, 24), (9, 24)]);
    }
    if kind == 0 || kind == 3 {
        tags.push((1, 8));
    } else {
        tags.push((14, if kind == 1 { 8 } else { 17 }));
        if kind == 1 {
            tags.push((1, 17));
        }
    }
    for (index, (tag, value)) in tags.into_iter().enumerate() {
        write_u64(&mut elf, 0x1100 + index * 16, tag)?;
        write_u64(&mut elf, 0x1108 + index * 16, value)?;
    }
    elf[0x1300..0x1319].copy_from_slice(b"\0answer\0scope.so\0base.so\0");
    write_u32(&mut elf, 0x1280, 1)?;
    write_u32(&mut elf, 0x1284, 2)?;
    write_u32(&mut elf, 0x1218, 1)?;
    elf[0x121c] = 0x12;
    write_u16(&mut elf, 0x121e, u16::from(kind != 0 && kind != 3))?;
    write_u64(
        &mut elf,
        0x1220,
        if kind == 0 || kind == 3 { 0 } else { 0x20 },
    )?;
    write_u64(&mut elf, 0x1228, 6)?;
    write_u64(&mut elf, 0x1380, base + 0x1000)?;
    write_u64(
        &mut elf,
        0x1388,
        if kind == 0 || kind == 3 {
            (1_u64 << 32) | 1
        } else {
            8
        },
    )?;
    if kind == 3 {
        write_u64(&mut elf, 0x1390, 0)?;
    } else {
        write_u64(&mut elf, 0x1390, if kind == 0 { 0 } else { 0x20 })?;
    }
    if kind == 0 || kind == 3 {
        // movabs rax, GOT; call [rax]; exit(rax). The call crosses images in
        // Ring 3; no kernel shortcut invokes the library implementation.
        let code = [
            0x48, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0x10, 0x48, 0x89, 0xc7, 0xb8, 3, 0, 0, 0,
            0xcd, 0x80, 0x0f, 0x0b,
        ];
        elf[0x1000..0x1000 + code.len()].copy_from_slice(&code);
        write_u64(&mut elf, 0x1002, base + 0x1000)?;
    } else {
        elf[0x1020..0x1026].copy_from_slice(&[0xb8, 42, 0, 0, 0, 0xc3]);
    }
    Some(elf)
}

fn package(bytes: &[u8]) -> Vec<u8> {
    let pair = ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::new(SEED));
    let signature = pair.sk.sign(bytes, None);
    let mut package = Vec::with_capacity(bytes.len() + 80);
    package.extend_from_slice(b"WAPP");
    package.extend_from_slice(&1_u16.to_le_bytes());
    package.extend_from_slice(&80_u16.to_le_bytes());
    package.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    package.extend_from_slice(&*signature);
    package.extend_from_slice(bytes);
    package
}

fn step(name: &str, passed: bool) -> bool {
    crate::serial::write_line(format_args!(
        "[S11.5] shared libraries {name}: {}",
        if passed { "PASSED" } else { "FAILED" }
    ));
    passed
}

#[cfg(feature = "stage11-compiler-test")]
pub(super) fn compiler_run() -> bool {
    // These are exact, unpatched Rust/LLD outputs. The harness retains their
    // input/output/tool hashes and supplies mandatory paths, never fallbacks.
    let main = include_bytes!(env!("WOVENHAT_COMPILER_MAIN_ELF"));
    let library = include_bytes!(env!("WOVENHAT_COMPILER_LIBRARY_ELF"));
    const PATH: &str = "/lib/libwoven-demo.so";
    if crate::vfs::stat(PATH).is_ok() {
        return false;
    }
    let made_lib = crate::vfs::stat("/lib").is_err();
    if made_lib && crate::vfs::mkdir("/lib").is_err() {
        return false;
    }
    let mut installed = false;
    let result = (|| -> Option<bool> {
        crate::vfs::create_read_only(PATH, &package(library)).ok()?;
        installed = true;
        let signed_main = package(main);
        drop(alloc::vec![0_u8; crate::vfs::NODE_CAPACITY * 3]);
        crate::heap::maintain_capacity();
        let frames = crate::memory::stats().allocated_frames;
        let processes = crate::task::process_count();
        let endpoints = crate::ipc::endpoint_count();
        let image = elf::parse(main).ok()?;
        if !image.is_position_independent() {
            return None;
        }
        for candidate in [0_u64, 0x30_000] {
            let program = if candidate == 0 {
                load_signed_elf_with_argv(&signed_main, &["compiler-shared-pie"])
            } else {
                // Offer the production graph loader a nonzero draw after the
                // same WAPP verification; never edit the linked ELF bytes.
                let verified = crate::signed_app::verify_package(&signed_main).ok()?;
                load_with_candidate_bias(verified.bytes(), &["compiler-shared-pie"], candidate)
            };
            let Some(program) = program else {
                crate::serial::write_line(format_args!(
                    "[S11.COMPILER] unmodified signed PIE load: FAILED bias={candidate:#x}"
                ));
                return None;
            };
            if Some(program.image.entry) != image.entry.checked_add(candidate) {
                let _ = destroy(program.address_space);
                return None;
            }
            let (pid, _) = crate::task::spawn_user_process("compiler-shared-pie", program).ok()?;
            let start = crate::timer::ticks();
            while !crate::task::process_exited(pid) {
                if crate::timer::ticks().wrapping_sub(start) > 500 {
                    return None;
                }
                crate::task::yield_now();
            }
            let status = crate::task::wait_process(pid.as_u64());
            let after = crate::memory::stats().allocated_frames;
            crate::serial::write_line(format_args!(
                "[S11.COMPILER] bias={candidate:#x} child status={:?} frames={frames}/{after}",
                status.as_ref().ok()
            ));
            if status != Ok(42)
                || after != frames
                || crate::task::process_count() != processes
                || crate::ipc::endpoint_count() != endpoints
                || crate::thread::reap_owner(pid.as_u64()) != 0
            {
                return None;
            }
        }
        crate::serial::write_line(format_args!(
            "[S11.COMPILER] actual shared import and exact owner teardown: PASSED"
        ));
        crate::serial::write_line(format_args!(
            "[S11.COMPILER] nonzero-bias shared PIE and teardown: PASSED"
        ));
        Some(true)
    })()
    .unwrap_or(false);
    let cleaned = !installed || crate::vfs::remove(PATH).is_ok();
    let directory_cleaned = !made_lib || crate::vfs::remove("/lib").is_ok();
    result && cleaned && directory_cleaned
}

pub(super) fn run() -> bool {
    let Some(main) = fixture(0) else {
        return false;
    };
    let Some(library) = fixture(1) else {
        return false;
    };
    let Some(base) = fixture(2) else {
        return false;
    };
    let Some(rel_main) = fixture(3) else {
        return false;
    };
    if crate::vfs::stat("/lib/scope.so").is_ok() || crate::vfs::stat("/lib/base.so").is_ok() {
        return false;
    }
    let made_lib = crate::vfs::stat("/lib").is_err();
    if made_lib && crate::vfs::mkdir("/lib").is_err() {
        return false;
    }
    let mut owned_scope = false;
    let mut owned_base = false;
    let result = (|| -> Option<bool> {
        crate::vfs::create_read_only("/lib/base.so", &package(&base)).ok()?;
        owned_base = true;
        // Unsigned ELF is refused even under /lib; path naming creates no trust.
        crate::vfs::create_read_only("/lib/scope.so", &library).ok()?;
        owned_scope = true;
        if !step(
            "unsigned dependency rejected",
            load(&main, &["shared-test"]).is_none(),
        ) {
            return None;
        }
        crate::vfs::remove("/lib/scope.so").ok()?;
        owned_scope = false;
        crate::vfs::write_file("/lib/scope.so", &package(&library)).ok()?;
        owned_scope = true;
        if !step(
            "writable dependency rejected",
            load(&main, &["shared-test"]).is_none(),
        ) {
            return None;
        }
        crate::vfs::remove("/lib/scope.so").ok()?;
        owned_scope = false;
        let mut tampered = package(&library);
        tampered[80 + 0x1021] ^= 1;
        crate::vfs::create_read_only("/lib/scope.so", &tampered).ok()?;
        owned_scope = true;
        if !step(
            "tampered signature rejected",
            load(&main, &["shared-test"]).is_none(),
        ) {
            return None;
        }
        crate::vfs::remove("/lib/scope.so").ok()?;
        owned_scope = false;
        let mut wrong_name = library.clone();
        wrong_name[0x1308..0x1310].copy_from_slice(b"other.so");
        crate::vfs::create_read_only("/lib/scope.so", &package(&wrong_name)).ok()?;
        owned_scope = true;
        if !step(
            "SONAME mismatch rejected",
            load(&main, &["shared-test"]).is_none(),
        ) {
            return None;
        }
        crate::vfs::remove("/lib/scope.so").ok()?;
        owned_scope = false;
        crate::vfs::create_read_only("/lib/scope.so", &package(&library)).ok()?;
        owned_scope = true;
        let mut traversal = main.clone();
        traversal[0x1308..0x1310].copy_from_slice(b"../ab.so");
        if !step(
            "path traversal rejected",
            load(&traversal, &["bad"]).is_none(),
        ) {
            return None;
        }
        drop(alloc::vec![0_u8; crate::vfs::NODE_CAPACITY*3]);
        crate::heap::maintain_capacity();
        let frames = crate::memory::stats().allocated_frames;
        let program = load(&main, &["shared-test"])?;
        let mut address = [0; 8];
        paging::read_user_bytes_in(
            program.address_space.paging,
            USER_REGION_START + 0x1000,
            &mut address,
        )
        .ok()?;
        if !step(
            "transitive graph and cross-image relocation",
            program.address_space.mapping_count == 6
                && u64::from_le_bytes(address) == LIBRARY_START + 0x20
                && program
                    .address_space
                    .contains_executable(LIBRARY_START + 0x20),
        ) {
            return None;
        }
        let mut relro_ok = true;
        let kernel_page = (crate::task::yield_now as *const () as u64) & !4095;
        relro_ok &= paging::destroy_user_address_space(
            program.address_space.paging,
            &[(kernel_page, 4096)],
        )
        .is_err();
        for range in program
            .address_space
            .relro
            .into_iter()
            .chain(program.address_space.library_relro.into_iter().flatten())
        {
            relro_ok &= !program
                .address_space
                .is_logically_writable(range.0, &[None; MAX_ANONYMOUS_MAPPINGS])
                && paging::user_range_has_protection_in(
                    program.address_space.paging,
                    range.0,
                    range.1,
                    false,
                    false,
                );
        }
        let clone = clone_address_space(program.address_space, &[None; MAX_ANONYMOUS_MAPPINGS])?;
        relro_ok &=
            !clone.is_logically_writable(LIBRARY_START + 0x1000, &[None; MAX_ANONYMOUS_MAPPINGS]);
        relro_ok &= destroy(clone);
        if !step(
            "W^X RELRO and fork preservation",
            relro_ok
                && paging::user_range_has_protection_in(
                    program.address_space.paging,
                    LIBRARY_START,
                    4096,
                    false,
                    true,
                ),
        ) {
            return None;
        }
        let (pid, _) = crate::task::spawn_user_process("shared-test", program).ok()?;
        let start = crate::timer::ticks();
        while !crate::task::process_exited(pid) {
            if crate::timer::ticks().wrapping_sub(start) > 500 {
                return None;
            }
            crate::task::yield_now();
        }
        let status = crate::task::wait_process(pid.as_u64());
        let after_frames = crate::memory::stats().allocated_frames;
        crate::serial::write_line(format_args!(
            "[S11.5] shared child status={:?} frames before={frames} after={after_frames}",
            status.as_ref().ok()
        ));
        if !step(
            "Ring-3 import call and owner teardown",
            status == Ok(42) && after_frames == frames,
        ) {
            return None;
        }
        let Some(rel_program) = load(&rel_main, &["rel-test"]) else {
            step("DT_REL runtime relocation", false);
            return None;
        };
        let mut rel_address = [0; 8];
        paging::read_user_bytes_in(
            rel_program.address_space.paging,
            USER_REGION_START + 0x1000,
            &mut rel_address,
        )
        .ok()?;
        if !step(
            "DT_REL runtime relocation",
            u64::from_le_bytes(rel_address) == LIBRARY_START + 0x20,
        ) {
            let _ = destroy(rel_program.address_space);
            return None;
        }
        let (rel_pid, _) = crate::task::spawn_user_process("rel-test", rel_program).ok()?;
        let rel_start = crate::timer::ticks();
        while !crate::task::process_exited(rel_pid) {
            if crate::timer::ticks().wrapping_sub(rel_start) > 500 {
                return None;
            }
            crate::task::yield_now();
        }
        if !step(
            "DT_REL Ring-3 execution and teardown",
            crate::task::wait_process(rel_pid.as_u64()) == Ok(42),
        ) {
            return None;
        }
        let mut unresolved = main.clone();
        unresolved[0x1301..0x1307].copy_from_slice(b"absent");
        if !step(
            "unresolved import rollback",
            load(&unresolved, &["bad"]).is_none()
                && crate::memory::stats().allocated_frames == frames,
        ) {
            return None;
        }
        let oversized = alloc::string::String::from_utf8(alloc::vec![b'x';256]).ok()?;
        if !step(
            "argv failure rollback",
            load(&main, &[&oversized]).is_none()
                && crate::memory::stats().allocated_frames == frames,
        ) {
            return None;
        }
        Some(true)
    })()
    .unwrap_or(false);
    let scope_cleaned = !owned_scope || crate::vfs::remove("/lib/scope.so").is_ok();
    let base_cleaned = !owned_base || crate::vfs::remove("/lib/base.so").is_ok();
    let cleaned = scope_cleaned && base_cleaned;
    let cleaned = cleaned && (!made_lib || crate::vfs::remove("/lib").is_ok());
    result && cleaned
}
