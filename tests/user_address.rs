//! Syscall pointer bounds: every user copy is limited to the canonical lower
//! half before the kernel walks page tables.
#[path = "../kernel/src/user_address.rs"]
mod user_address;

use user_address::{RangeError, USER_ADDRESS_LIMIT, check_user_range};

#[test]
fn accepts_ranges_inside_user_space() {
    assert_eq!(check_user_range(0x40_0000, 4096), Ok(()));
    assert_eq!(check_user_range(0, 1), Ok(()));
    // The last user byte, and a range that ends exactly at the limit.
    assert_eq!(check_user_range(USER_ADDRESS_LIMIT - 1, 1), Ok(()));
    assert_eq!(check_user_range(USER_ADDRESS_LIMIT - 4096, 4096), Ok(()));
}

#[test]
fn empty_ranges_touch_nothing_and_are_accepted() {
    assert_eq!(check_user_range(0, 0), Ok(()));
    assert_eq!(check_user_range(u64::MAX, 0), Ok(()));
}

#[test]
fn rejects_non_canonical_addresses() {
    // Each of these would panic in `VirtAddr::new`.
    for start in [
        USER_ADDRESS_LIMIT,
        0x0001_0000_0000_0000,
        0x8000_0000_0000_0000,
        0xFFFF_7FFF_FFFF_FFFF,
    ] {
        assert_eq!(check_user_range(start, 1), Err(RangeError::OutsideUser));
    }
}

#[test]
fn rejects_kernel_half_addresses() {
    assert_eq!(
        check_user_range(0xFFFF_8000_0000_0000, 8),
        Err(RangeError::OutsideUser)
    );
    assert_eq!(
        check_user_range(0xFFFF_FFFF_8000_0000, 4096),
        Err(RangeError::OutsideUser)
    );
}

#[test]
fn rejects_ranges_that_cross_out_of_user_space() {
    assert_eq!(
        check_user_range(USER_ADDRESS_LIMIT - 1, 2),
        Err(RangeError::OutsideUser)
    );
    assert_eq!(
        check_user_range(USER_ADDRESS_LIMIT - 4096, 4097),
        Err(RangeError::OutsideUser)
    );
}

#[test]
fn rejects_wrapping_ranges() {
    assert_eq!(check_user_range(u64::MAX, 1), Err(RangeError::Overflow));
    assert_eq!(check_user_range(1, usize::MAX), Err(RangeError::Overflow));
}
