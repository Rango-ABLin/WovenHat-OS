#[path = "../kernel/src/hal/pci/bar.rs"]
mod bar;

use bar::{decode_probe, encode, Error, Kind};

#[test]
fn decodes_io_and_memory32_masks() {
    let io = decode_probe(0x0000_0001, 0xffff_ff01, None).unwrap();
    assert_eq!(io.kind, Kind::Io);
    assert_eq!(io.size, 0x100);

    let mem = decode_probe(0x0000_0008, 0xffff_f008, None).unwrap();
    assert_eq!(mem.kind, Kind::Memory32);
    assert_eq!(mem.size, 0x1000);
    assert!(mem.prefetchable);
}

#[test]
fn decodes_64_bit_bar_mask() {
    let probe = decode_probe(0x0000_000c, 0xffe0_000c, Some(0xffff_ffff)).unwrap();
    assert_eq!(probe.kind, Kind::Memory64);
    assert_eq!(probe.size, 0x20_0000);
    let (low, high) = encode(probe, 0x1_2000_0000).unwrap();
    assert_eq!(low & !0xf, 0x2000_0000);
    assert_eq!(high, Some(1));
}

#[test]
fn rejects_reserved_memory_type_and_non_power_of_two_mask() {
    assert_eq!(decode_probe(0x2, 0xffff_f002, None), Err(Error::UnsupportedMemoryType));
    assert_eq!(decode_probe(0, 0xffff_e008, None), Err(Error::InvalidMask));
}

#[test]
fn encoding_rejects_misalignment_and_32_bit_overflow() {
    let probe = decode_probe(0, 0xffff_f000, None).unwrap();
    assert_eq!(encode(probe, 0x1800), Err(Error::AddressOverflow));
    assert_eq!(encode(probe, 0x1_0000_0000), Err(Error::AddressOverflow));
}
