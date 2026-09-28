#[path = "../kernel/src/hal/pci/bridge.rs"]
mod bridge;

use bridge::{encode_io, encode_memory, encode_prefetch, Error, Window};

#[test]
fn encodes_32_bit_io_window() {
    let encoded = encode_io(Window { base: 0x1000, size: 0x2000 }).unwrap();
    assert_eq!(encoded.low & 0x101, 0x101);
    assert_eq!(encoded.upper & 0xffff, 0);
}

#[test]
fn encodes_non_prefetchable_memory_window() {
    let encoded = encode_memory(Window { base: 0x8000_0000, size: 0x0200_0000 }).unwrap();
    assert_eq!(encoded.value & 0x0000_fff0, 0x8000);
    assert_eq!(encoded.value & 0xfff0_0000, 0x81f0_0000);
}

#[test]
fn encodes_64_bit_prefetchable_window() {
    let encoded = encode_prefetch(Window { base: 0x1_2000_0000, size: 0x0200_0000 }).unwrap();
    assert_eq!(encoded.base_upper, 1);
    assert_eq!(encoded.limit_upper, 1);
    assert_eq!(encoded.low & 0x0001_0001, 0x0001_0001);
}

#[test]
fn rejects_bad_granularity_and_width() {
    assert_eq!(encode_io(Window { base: 0x1800, size: 0x1000 }), Err(Error::Misaligned));
    assert_eq!(encode_memory(Window { base: 0x8000_0000, size: 0x180000 }), Err(Error::Misaligned));
    assert_eq!(encode_memory(Window { base: 0x1_0000_0000, size: 0x100000 }), Err(Error::AddressWidth));
    assert_eq!(encode_prefetch(Window { base: 0, size: 0 }), Err(Error::Empty));
}


#[test]
fn composes_all_windows_into_one_register_image() {
    let registers = bridge::encode_windows(bridge::Windows {
        io: Some(Window { base: 0x4000, size: 0x4000 }),
        memory: Some(Window { base: 0x9000_0000, size: 0x0100_0000 }),
        prefetch: Some(Window { base: 0x2_0000_0000, size: 0x0200_0000 }),
    }).unwrap();
    assert_ne!(registers.io_low, 0);
    assert_ne!(registers.memory, 0);
    assert_ne!(registers.prefetch_low, 0);
    assert_eq!(registers.prefetch_base_upper, 2);
    assert_eq!(registers.prefetch_limit_upper, 2);
}

#[test]
fn absent_windows_encode_disabled_registers() {
    let registers = bridge::encode_windows(bridge::Windows::default()).unwrap();
    assert_eq!(registers, bridge::Registers::default());
}

#[test]
fn composite_encoding_fails_before_partial_register_image() {
    assert_eq!(
        bridge::encode_windows(bridge::Windows {
            io: Some(Window { base: 0x1000, size: 0x1000 }),
            memory: Some(Window { base: 0x8008_0000, size: 0x100000 }),
            prefetch: None,
        }),
        Err(Error::Misaligned)
    );
}
