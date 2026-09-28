// Host-side MSI-X layout tests use a compact pure model matching the PCI
// table-entry rules; device/config-space paths are exercised by the kernel
// Stage 13.2 self-test.
const ENTRY_BYTES: u64 = 16;
fn entry_offset(base: u32, table_size: u16, index: u16) -> Option<u64> {
    if index >= table_size { return None; }
    u64::from(base).checked_add(u64::from(index).checked_mul(ENTRY_BYTES)?)
}
fn pba_bytes(table_size: u16) -> u64 { u64::from(table_size).div_ceil(64) * 8 }

#[test]
fn table_entries_are_sixteen_byte_bounded_records() {
    assert_eq!(entry_offset(0x2000, 4, 0), Some(0x2000));
    assert_eq!(entry_offset(0x2000, 4, 3), Some(0x2030));
    assert_eq!(entry_offset(0x2000, 4, 4), None);
}
#[test]
fn pending_bit_array_rounds_to_qword_groups() {
    assert_eq!(pba_bytes(1), 8);
    assert_eq!(pba_bytes(64), 8);
    assert_eq!(pba_bytes(65), 16);
    assert_eq!(pba_bytes(2048), 256);
}
