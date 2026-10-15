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
fn dependency_order_and_duplicate_needed_are_checked() {
    let main_bytes = fixture(&[8, 13]);
    let main = image(&main_bytes);
    let library_bytes = fixture(&[]);
    let library = image(&library_bytes);
    let mut scope = elf::SymbolScope::new(&main, &main_bytes);
    assert!(
        scope
            .add_dependency(b"b.so", &library, &library_bytes)
            .is_ok()
    );
    assert!(
        scope
            .add_dependency(b"a.so", &library, &library_bytes)
            .is_ok()
    );
    assert!(scope.validate_dependencies().is_err());
    let duplicate_bytes = fixture(&[8, 8]);
    let duplicate = image(&duplicate_bytes);
    let mut scope = elf::SymbolScope::new(&duplicate, &duplicate_bytes);
    assert!(
        scope
            .add_dependency(b"a.so", &library, &library_bytes)
            .is_ok()
    );
    assert!(
        scope
            .add_dependency(b"b.so", &library, &library_bytes)
            .is_ok()
    );
    assert!(scope.validate_dependencies().is_err());
}
