//! x86-64 ancestor permissions for leaf-controlled user mappings.
const PRESENT: u64 = 1;
const WRITABLE: u64 = 1 << 1;
const USER: u64 = 1 << 2;

/// User mappings may share page-table ancestors with other leaf policies.
/// A read-only user leaf must not make the whole ancestor subtree read-only:
/// COW and mprotect may later grant that leaf its logical write permission.
/// This never changes the leaf or adds USER to a supervisor-only mapping.
pub const fn ancestor_bits(leaf: u64) -> u64 {
    let inherited = leaf & (PRESENT | WRITABLE | USER);
    if leaf & (PRESENT | USER) == (PRESENT | USER) {
        inherited | WRITABLE
    } else {
        inherited
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_read_only_leaves_do_not_block_future_cow_writes() {
        for leaf in [PRESENT | USER, PRESENT | USER | (1 << 63)] {
            assert_eq!(ancestor_bits(leaf), PRESENT | WRITABLE | USER);
            assert_eq!(leaf & WRITABLE, 0, "the leaf is still read-only");
        }
    }

    #[test]
    fn supervisor_and_leaf_specific_permissions_are_not_broadened() {
        for bits in 0..8 {
            let result = ancestor_bits(bits | (1 << 63) | (1 << 8));
            assert_eq!(result & USER, bits & USER);
            assert_eq!(result & PRESENT, bits & PRESENT);
            assert_eq!(result & !7, 0);
            if bits & (PRESENT | USER) != (PRESENT | USER) {
                assert_eq!(result & WRITABLE, bits & WRITABLE);
            }
        }
    }
}
