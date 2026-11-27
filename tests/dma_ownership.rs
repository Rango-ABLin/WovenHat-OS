#[path = "../kernel/src/dma/ownership.rs"]
mod ownership;

use ownership::{Error, Owner, Ownership, check_access, check_device_range};

#[test]
fn buffer_starts_cpu_owned_and_alternates_strictly() {
    let mut state = Ownership::new_cpu();
    assert_eq!(state.owner(), Owner::Cpu);
    assert_eq!(
        state.reclaim(),
        Err(Error::WrongOwner {
            expected: Owner::Device,
            actual: Owner::Cpu
        })
    );
    state.hand_to_device().unwrap();
    assert_eq!(state.owner(), Owner::Device);
    assert_eq!(
        state.hand_to_device(),
        Err(Error::WrongOwner {
            expected: Owner::Cpu,
            actual: Owner::Device
        })
    );
    state.reclaim().unwrap();
    assert_eq!(state.owner(), Owner::Cpu);
}

#[test]
fn rejected_transition_leaves_owner_unchanged() {
    let mut state = Ownership::new_cpu();
    state.hand_to_device().unwrap();
    let before = state;
    assert!(state.hand_to_device().is_err());
    assert_eq!(state, before);
}

#[test]
fn access_accepts_full_buffer_and_last_aligned_slot() {
    assert_eq!(check_access(4096, 0, 4096, 4), Ok(()));
    assert_eq!(check_access(4096, 4092, 4, 4), Ok(()));
}

#[test]
fn access_rejects_past_end_and_overflowing_offsets() {
    assert_eq!(check_access(4096, 4093, 4, 1), Err(Error::OutOfBounds));
    assert_eq!(check_access(4096, 4096, 4, 4), Err(Error::OutOfBounds));
    assert_eq!(
        check_access(4096, usize::MAX - 1, 4, 1),
        Err(Error::OutOfBounds)
    );
}

#[test]
fn access_rejects_misalignment_empty_width_and_bad_alignment() {
    assert_eq!(check_access(4096, 2, 4, 4), Err(Error::Misaligned));
    assert_eq!(check_access(4096, 0, 0, 4), Err(Error::EmptyRange));
    assert_eq!(check_access(4096, 0, 4, 0), Err(Error::Misaligned));
    assert_eq!(check_access(4096, 0, 4, 3), Err(Error::Misaligned));
}

#[test]
fn device_range_accepts_ranges_within_limit() {
    assert_eq!(check_device_range(0x1000, 4096, 0x1_0000_0000), Ok(0x2000));
    assert_eq!(
        check_device_range(0xffff_f000, 0x1000, 0x1_0000_0000),
        Ok(0x1_0000_0000)
    );
}

#[test]
fn device_range_rejects_32_bit_overflow_and_empty() {
    assert_eq!(
        check_device_range(0xffff_f000, 0x1001, 0x1_0000_0000),
        Err(Error::AddressOutOfRange)
    );
    assert_eq!(
        check_device_range(0, 0, 0x1_0000_0000),
        Err(Error::EmptyRange)
    );
}

#[test]
fn device_range_rejects_address_arithmetic_overflow() {
    assert_eq!(
        check_device_range(u64::MAX - 1, 16, u64::MAX),
        Err(Error::AddressOverflow)
    );
}
