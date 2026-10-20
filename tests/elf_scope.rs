#[path = "../kernel/src/config.rs"]
pub mod config;
#[path = "../kernel/src/elf.rs"]
pub mod elf;

const BASE: u64 = 0x4000_0000_0000;
const DYNAMIC: usize = 0x1100;
const SYMBOL: usize = 0x1218;

fn put16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

#[test]
fn fixed_executables_ignore_even_overflowing_aslr_candidates() {
    let mut bytes = fixture(&[]);
    put16(&mut bytes, 16, 2);
    put16(&mut bytes, 56, 1); // static ET_EXEC, no PT_DYNAMIC
    for candidate in [0, 0x30_000, u64::MAX] {
        let image = elf::parse(&bytes).unwrap_or_else(|_| panic!("static fixture"));
        assert!(!image.is_position_independent());
        let image = image.relocated_for_execution(candidate).unwrap();
        assert_eq!(image.entry, BASE);
        assert_eq!(image.load_bias(), 0);
        assert_eq!(image.segments().next().unwrap().mapping_start, BASE);
    }
    assert!(matches!(
        elf::parse_shared(&bytes),
        Err(elf::Error::InvalidHeader)
    ));
}

#[test]
fn position_independent_executables_keep_nonzero_aslr() {
    let bytes = fixture(&[]);
    let image = elf::parse(&bytes).unwrap_or_else(|_| panic!("PIE fixture"));
    assert!(image.is_position_independent());
    let image = image.relocated_for_execution(0x30_000).unwrap();
    assert_eq!(image.entry, BASE + 0x30_000);
    assert_eq!(image.load_bias(), 0x30_000);
    assert_eq!(
        image.segments().next().unwrap().mapping_start,
        BASE + 0x30_000
    );
}

#[test]
fn null_debug_rendezvous_metadata_is_inert() {
    let mut bytes = fixture(&[]);
    put64(&mut bytes, 152, 7 * 16);
    put64(&mut bytes, 160, 7 * 16);
    put64(&mut bytes, DYNAMIC + 5 * 16, 21);
    let parsed = image(&bytes);
    assert_eq!(parsed.entry, BASE);
    assert_eq!(
        parsed.defined_global_symbol(&bytes, b"export"),
        Some(BASE + 0x20)
    );
    put64(&mut bytes, 24, 0);
    assert!(elf::parse_shared(&bytes).is_ok());
}

#[test]
fn nonzero_debug_rendezvous_addresses_are_not_trusted_or_followed() {
    for value in [1, BASE, u64::MAX] {
        let mut bytes = fixture(&[]);
        put64(&mut bytes, 152, 7 * 16);
        put64(&mut bytes, 160, 7 * 16);
        put64(&mut bytes, DYNAMIC + 5 * 16, 21);
        put64(&mut bytes, DYNAMIC + 5 * 16 + 8, value);
        assert!(matches!(elf::parse(&bytes), Err(elf::Error::Unsupported)));
    }
}

#[test]
fn duplicate_debug_metadata_is_rejected() {
    let mut bytes = fixture(&[]);
    put64(&mut bytes, 152, 8 * 16);
    put64(&mut bytes, 160, 8 * 16);
    put64(&mut bytes, DYNAMIC + 5 * 16, 21);
    put64(&mut bytes, DYNAMIC + 6 * 16, 21);
    assert!(elf::parse(&bytes).is_err());
}

// A real ELF64 ET_DYN with RX text, a dynamic table, SysV symbol metadata,
// and one exported function. Each test goes through the production parser.
fn fixture(dependencies: &[u64]) -> Vec<u8> {
    let mut bytes = vec![0; 0x1400];
    bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    put16(&mut bytes, 16, 3);
    put16(&mut bytes, 18, 0x3e);
    put32(&mut bytes, 20, 1);
    put64(&mut bytes, 24, BASE);
    put64(&mut bytes, 32, 64);
    put16(&mut bytes, 52, 64);
    put16(&mut bytes, 54, 56);
    put16(&mut bytes, 56, 2);
    put32(&mut bytes, 64, 1);
    put32(&mut bytes, 68, 5);
    put64(&mut bytes, 72, 0x1000);
    put64(&mut bytes, 80, BASE);
    put64(&mut bytes, 96, 0x400);
    put64(&mut bytes, 104, 0x1000);
    put64(&mut bytes, 112, 0x1000);
    put32(&mut bytes, 120, 2);
    put64(&mut bytes, 128, DYNAMIC as u64);
    put64(&mut bytes, 136, BASE + 0x100);
    let dynamic_size = (6 + dependencies.len()) * 16;
    put64(&mut bytes, 152, dynamic_size as u64);
    put64(&mut bytes, 160, dynamic_size as u64);
    for (index, (tag, value)) in [
        (4, BASE + 0x280),
        (5, BASE + 0x300),
        (10, 18),
        (6, BASE + 0x200),
        (11, 24),
    ]
    .into_iter()
    .chain(dependencies.iter().map(|offset| (1, *offset)))
    .enumerate()
    {
        put64(&mut bytes, DYNAMIC + index * 16, tag);
        put64(&mut bytes, DYNAMIC + index * 16 + 8, value);
    }
    bytes[0x1300..0x1312].copy_from_slice(b"\0export\0a.so\0b.so\0");
    put32(&mut bytes, 0x1280, 1);
    put32(&mut bytes, 0x1284, 2);
    put32(&mut bytes, SYMBOL, 1);
    bytes[SYMBOL + 4] = 0x12;
    put16(&mut bytes, SYMBOL + 6, 1);
    put64(&mut bytes, SYMBOL + 8, BASE + 0x20);
    put64(&mut bytes, SYMBOL + 16, 8);
    bytes
}

fn image(bytes: &[u8]) -> elf::Image {
    elf::parse(bytes).unwrap_or_else(|_| panic!("fixture must parse"))
}

fn tls_fixture() -> Vec<u8> {
    let mut bytes = fixture(&[]);
    put16(&mut bytes, 56, 3);
    put32(&mut bytes, 176, 7);
    put64(&mut bytes, 184, 0x1008);
    put64(&mut bytes, 192, BASE + 8);
    put64(&mut bytes, 208, 8);
    put64(&mut bytes, 216, 136);
    put64(&mut bytes, 224, 64);
    bytes
}

fn plt_fixture() -> Vec<u8> {
    let mut bytes = fixture(&[]);
    bytes.resize(0x2200, 0);
    put16(&mut bytes, 56, 3);
    // A separate writable PT_LOAD contains the eager PLT relocation target
    // and the file-backed JMPREL table.
    put32(&mut bytes, 176, 1);
    put32(&mut bytes, 180, 6);
    put64(&mut bytes, 184, 0x2000);
    put64(&mut bytes, 192, BASE + 0x1000);
    put64(&mut bytes, 208, 0x100);
    put64(&mut bytes, 216, 0x100);
    put64(&mut bytes, 224, 0x1000);
    put64(&mut bytes, 152, 10 * 16);
    put64(&mut bytes, DYNAMIC + 5 * 16, 2); // DT_PLTRELSZ
    put64(&mut bytes, DYNAMIC + 5 * 16 + 8, 24);
    put64(&mut bytes, DYNAMIC + 6 * 16, 20); // DT_PLTREL = DT_RELA
    put64(&mut bytes, DYNAMIC + 6 * 16 + 8, 7);
    put64(&mut bytes, DYNAMIC + 7 * 16, 23); // DT_JMPREL
    put64(&mut bytes, DYNAMIC + 7 * 16 + 8, BASE + 0x1000);
    put64(&mut bytes, DYNAMIC + 8 * 16, 9); // DT_RELAENT
    put64(&mut bytes, DYNAMIC + 8 * 16 + 8, 24);
    put64(&mut bytes, 0x2000, BASE + 0x1020);
    put64(
        &mut bytes,
        0x2008,
        (1_u64 << 32) | elf::R_X86_64_JUMP_SLOT as u64,
    );
    bytes
}

#[test]
fn eager_plt_rela_metadata_is_bounded_and_file_backed() {
    let bytes = plt_fixture();
    let parsed = image(&bytes);
    let dynamic = parsed.dynamic().expect("dynamic metadata");
    assert_eq!(dynamic.plt_rela_size, 24);
    assert_eq!(dynamic.plt_rela_file_offset, 0x2000);
    assert_eq!(dynamic.plt_rela_address, BASE + 0x1000);
}

#[test]
fn plt_rela_requires_rela_encoding_and_writable_targets() {
    let mut wrong_encoding = plt_fixture();
    put64(&mut wrong_encoding, DYNAMIC + 6 * 16 + 8, 17);
    assert!(matches!(
        elf::parse(&wrong_encoding),
        Err(elf::Error::Unsupported)
    ));

    let mut read_only_target = plt_fixture();
    put64(&mut read_only_target, 0x2000, BASE + 0x20);
    assert!(matches!(
        elf::parse(&read_only_target),
        Err(elf::Error::InvalidSegment)
    ));
}

#[test]
fn tls_source_congruence_and_address_survive_image_relocation() {
    let bytes = tls_fixture();
    let parsed = image(&bytes);
    let tls = parsed.tls().unwrap();
    assert_eq!(tls.virtual_address, BASE + 8);
    assert_eq!(tls.file_offset, 0x1008);
    assert_eq!(tls.alignment, 64);
    assert_eq!(
        parsed
            .relocated(0x30000)
            .unwrap()
            .tls()
            .unwrap()
            .virtual_address,
        BASE + 0x30008
    );
}

#[test]
fn tls_initializer_requires_exact_file_backed_source() {
    for (field, value) in [(184, 0x1208), (192, BASE + 0x48), (208, 0x401)] {
        let mut bytes = tls_fixture();
        put64(&mut bytes, field, value);
        assert!(elf::parse(&bytes).is_err());
    }
    let mut bss = tls_fixture();
    put64(&mut bss, 208, 0);
    assert!(elf::parse(&bss).is_ok());
}

#[test]
fn tls_overflow_alignment_and_duplicate_headers_fail_closed() {
    for (field, value) in [
        (192, u64::MAX),
        (216, u64::MAX),
        (224, 0),
        (224, 3),
        (224, 8192),
    ] {
        let mut bytes = tls_fixture();
        put64(&mut bytes, field, value);
        assert!(elf::parse(&bytes).is_err());
    }
    let mut duplicate = tls_fixture();
    put16(&mut duplicate, 56, 4);
    let header = duplicate[176..232].to_vec();
    duplicate[232..288].copy_from_slice(&header);
    assert!(elf::parse(&duplicate).is_err());
}

#[test]
fn exported_symbol_tracks_aslr_once() {
    let bytes = fixture(&[]);
    let image = image(&bytes).relocated(0x8000).unwrap();
    assert_eq!(
        image.defined_global_symbol(&bytes, b"export"),
        Some(BASE + 0x8020)
    );
}

#[test]
fn visible_exports_must_fit_completely_inside_a_load_segment() {
    for visibility in [0, 3] {
        let mut bytes = fixture(&[]);
        bytes[SYMBOL + 5] = visibility;
        assert_eq!(
            image(&bytes).defined_global_symbol(&bytes, b"export"),
            Some(BASE + 0x20)
        );
        put64(&mut bytes, SYMBOL + 8, BASE + 0xffc);
        assert_eq!(image(&bytes).defined_global_symbol(&bytes, b"export"), None);
        put64(&mut bytes, SYMBOL + 8, u64::MAX);
        assert_eq!(image(&bytes).defined_global_symbol(&bytes, b"export"), None);
    }
}

#[test]
fn valid_scope_resolves_in_order_and_refuses_incomplete_or_extra_images() {
    let mut main_bytes = fixture(&[8, 13]);
    put16(&mut main_bytes, SYMBOL + 6, 0);
    let main = image(&main_bytes);
    let bytes = fixture(&[]);
    let first = image(&bytes).relocated(0x8000).unwrap();
    let second = image(&bytes).relocated(0x10000).unwrap();
    let mut scope = elf::SymbolScope::new(&main, &main_bytes);
    assert_eq!(scope.resolve(b"export"), None);
    assert!(scope.add_dependency(b"a.so", &first, &bytes).is_ok());
    assert_eq!(scope.resolve(b"export"), None);
    assert!(scope.add_dependency(b"b.so", &second, &bytes).is_ok());
    assert!(scope.validate_dependencies().is_ok());
    assert_eq!(scope.resolve(b"export"), Some(BASE + 0x8020));
    assert_eq!(scope.resolve(b"expor"), None);
    assert!(scope.add_dependency(b"extra.so", &second, &bytes).is_ok());
    assert_eq!(scope.resolve(b"export"), None);
}

#[test]
fn main_image_definition_precedes_libraries() {
    let main_bytes = fixture(&[8]);
    let main = image(&main_bytes).relocated(0x4000).unwrap();
    let bytes = fixture(&[]);
    let library = image(&bytes).relocated(0x8000).unwrap();
    let mut scope = elf::SymbolScope::new(&main, &main_bytes);
    assert!(scope.add_dependency(b"a.so", &library, &bytes).is_ok());
    assert_eq!(scope.resolve(b"export"), Some(BASE + 0x4020));
}

#[test]
fn local_relocations_apply_bias_and_allow_private_definitions() {
    let mut bytes = fixture(&[]);
    bytes[SYMBOL + 4] = 0x02;
    bytes[SYMBOL + 5] = 2;
    let relocated = image(&bytes).relocated(0x9000).unwrap();
    assert_eq!(
        relocated.defined_symbol_value(&bytes, 1),
        Some(BASE + 0x9020)
    );
    assert_eq!(relocated.defined_global_symbol(&bytes, b"export"), None);
    assert_eq!(relocated.defined_symbol_value(&bytes, 0), None);
    assert_eq!(relocated.defined_symbol_value(&bytes, 2), None);
    assert_eq!(
        relocated.defined_symbol_value(&bytes[..SYMBOL + 8], 1),
        None
    );
}

#[test]
fn hidden_internal_tls_and_reserved_section_symbols_are_not_exports() {
    for (info, visibility, section) in [
        (0x12, 1, 1),
        (0x12, 2, 1),
        (0x16, 0, 1),
        (0x1a, 0, 1),
        (0x12, 0, 0xfff1),
        (0x12, 0, 0),
    ] {
        let mut bytes = fixture(&[]);
        bytes[SYMBOL + 4] = info;
        bytes[SYMBOL + 5] = visibility;
        put16(&mut bytes, SYMBOL + 6, section);
        assert_eq!(image(&bytes).defined_global_symbol(&bytes, b"export"), None);
    }
}

#[test]
fn needed_index_stops_at_declared_count() {
    let bytes = fixture(&[8]);
    let image = image(&bytes);
    assert_eq!(image.needed_name(&bytes, 0), Some(b"a.so".as_slice()));
    assert_eq!(image.needed_name(&bytes, 1), None);
}

#[test]
fn shared_objects_have_a_distinct_entry_and_type_contract() {
    let mut bytes = fixture(&[]);
    put64(&mut bytes, 24, 0);
    assert!(elf::parse(&bytes).is_err());
    assert!(elf::parse_shared(&bytes).is_ok());
    put16(&mut bytes, 16, 2);
    assert!(elf::parse_shared(&bytes).is_err());
}

#[test]
fn transitive_cycles_share_one_bounded_namespace() {
    let main_bytes = fixture(&[8]);
    let a_bytes = fixture(&[13]);
    let b_bytes = fixture(&[8]);
    let main = image(&main_bytes);
    let a = image(&a_bytes);
    let b = image(&b_bytes);
    let mut scope = elf::SymbolScope::new(&main, &main_bytes);
    assert!(scope.add_dependency(b"a.so", &a, &a_bytes).is_ok());
    assert!(scope.validate_dependencies().is_err());
    assert!(scope.add_dependency(b"b.so", &b, &b_bytes).is_ok());
    assert!(scope.validate_dependencies().is_ok());
    assert_eq!(scope.resolve(b"export"), Some(BASE + 0x20));
}

#[test]
fn undefined_imports_use_scope_and_protected_definitions_bind_locally() {
    let mut main_bytes = fixture(&[8]);
    put16(&mut main_bytes, SYMBOL + 6, 0);
    let main = image(&main_bytes);
    let mut bytes = fixture(&[]);
    bytes[SYMBOL + 5] = 3;
    let library = image(&bytes).relocated(0x8000).unwrap();
    let mut scope = elf::SymbolScope::new(&main, &main_bytes);
    assert!(scope.add_dependency(b"a.so", &library, &bytes).is_ok());
    assert_eq!(
        main.relocation_symbol_value(&main_bytes, 1, &scope),
        Some(BASE + 0x8020)
    );
    assert_eq!(
        library.relocation_symbol_value(&bytes, 1, &scope),
        Some(BASE + 0x8020)
    );
}

#[test]
fn versioned_metadata_is_rejected_before_unversioned_resolution() {
    for tag in [
        0x6fff_fff0,
        0x6fff_fffc,
        0x6fff_fffd,
        0x6fff_fffe,
        0x6fff_ffff,
    ] {
        let mut bytes = fixture(&[]);
        put64(&mut bytes, DYNAMIC + 5 * 16, tag);
        put64(&mut bytes, DYNAMIC + 5 * 16 + 8, BASE + 0x300);
        put64(&mut bytes, 152, 7 * 16);
        put64(&mut bytes, 160, 7 * 16);
        assert!(elf::parse(&bytes).is_err());
    }
}

#[test]
fn dependency_order_and_duplicate_needed_are_checked() {
    let main_bytes = fixture(&[8, 13]);
    let main = image(&main_bytes);
    let library_bytes = fixture(&[]);
    let library = image(&library_bytes);
    let mut scope = elf::SymbolScope::new(&main, &main_bytes);
    assert!(scope
        .add_dependency(b"b.so", &library, &library_bytes)
        .is_ok());
    assert!(scope
        .add_dependency(b"a.so", &library, &library_bytes)
        .is_ok());
    assert!(scope.validate_dependencies().is_err());
    let duplicate_bytes = fixture(&[8, 8]);
    let duplicate = image(&duplicate_bytes);
    let mut scope = elf::SymbolScope::new(&duplicate, &duplicate_bytes);
    assert!(scope
        .add_dependency(b"a.so", &library, &library_bytes)
        .is_ok());
    assert!(scope
        .add_dependency(b"b.so", &library, &library_bytes)
        .is_ok());
    assert!(scope.validate_dependencies().is_err());
}

#[test]
fn every_truncated_file_backed_fixture_is_rejected() {
    let bytes = fixture(&[8, 13]);
    assert!(elf::parse(&bytes).is_ok());
    assert!(elf::parse_shared(&bytes).is_ok());
    for length in 0..bytes.len() {
        assert!(
            elf::parse(&bytes[..length]).is_err(),
            "main prefix {length}"
        );
        assert!(
            elf::parse_shared(&bytes[..length]).is_err(),
            "shared prefix {length}"
        );
    }
}

#[test]
fn overflowing_file_and_dynamic_metadata_is_rejected() {
    for offset in [
        32,
        72,
        80,
        96,
        104,
        128,
        136,
        152,
        DYNAMIC + 8,
        DYNAMIC + 24,
        DYNAMIC + 40,
        DYNAMIC + 56,
        DYNAMIC + 72,
        DYNAMIC + 88,
    ] {
        let mut bytes = fixture(&[8]);
        put64(&mut bytes, offset, u64::MAX);
        assert!(elf::parse(&bytes).is_err(), "main metadata at {offset:#x}");
        assert!(
            elf::parse_shared(&bytes).is_err(),
            "shared metadata at {offset:#x}"
        );
    }
}

#[test]
fn deterministic_metadata_mutations_never_panic_in_parser_or_resolver() {
    let original = fixture(&[8, 13]);
    let offsets = (0..176)
        .chain(DYNAMIC..DYNAMIC + 8 * 16)
        .chain(SYMBOL..SYMBOL + 24)
        .chain(0x1280..0x1290)
        .chain(0x1300..0x1312);
    for offset in offsets {
        for mask in [1, 0x80, 0xff] {
            let mut bytes = original.clone();
            bytes[offset] ^= mask;
            let result = std::panic::catch_unwind(|| {
                for image in [elf::parse(&bytes), elf::parse_shared(&bytes)]
                    .into_iter()
                    .flatten()
                {
                    for index in [0, 1, 4, usize::MAX] {
                        let _ = image.needed_name(&bytes, index);
                        let _ = image.defined_symbol_value(&bytes, index);
                    }
                    let _ = image.soname(&bytes);
                    let _ = image.defined_global_symbol(&bytes, b"export");
                    let scope = elf::SymbolScope::new(&image, &bytes);
                    let _ = scope.validate_dependencies();
                    let _ = scope.resolve(b"export");
                    let _ = image.relocation_symbol_value(&bytes, usize::MAX, &scope);
                    let _ = image.relocated(0x1000);
                }
            });
            assert!(result.is_ok(), "mutation at {offset:#x}, XOR {mask:#x}");
        }
    }
}
