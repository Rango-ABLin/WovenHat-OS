#[path = "../kernel/src/userspace/tls_layout.rs"]
mod tls_layout;
use tls_layout::Layout;

#[test]
fn real_compiler_layouts_place_data_before_a_mapped_self_pointer() {
    let simple = Layout::new(8, 8, 8, 0x400000002200, 65536).unwrap();
    assert_eq!(
        (
            simple.image_offset,
            simple.thread_pointer_offset,
            simple.mapping_size
        ),
        (0, 8, 4096)
    );
    let aligned = Layout::new(128, 136, 64, 0x400000002500, 65536).unwrap();
    assert_eq!(
        (aligned.image_offset, aligned.thread_pointer_offset),
        (0, 192)
    );
    let page = Layout::new(4096, 4096, 4096, 0, 65536).unwrap();
    assert_eq!(
        (page.thread_pointer_offset, page.mapping_size),
        (4096, 8192)
    );
}

#[test]
fn every_alignment_and_congruence_preserves_bounds() {
    for alignment in [1_u64, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096] {
        for congruence in 0..alignment {
            for memory in [1, 7, 8, 136, 4095, 4096, 4097] {
                let layout = Layout::new(1, memory, alignment, congruence, 65536).unwrap();
                assert_eq!(layout.image_offset as u64 % alignment, congruence);
                assert_eq!(layout.thread_pointer_offset % alignment.max(8) as usize, 0);
                assert!(layout.image_offset + memory <= layout.thread_pointer_offset);
                assert!(layout.thread_pointer_offset + 8 <= layout.mapping_size);
                assert!(layout.mapping_size <= 65536);
            }
        }
    }
}

#[test]
fn invalid_overflowing_and_over_capacity_layouts_fail_closed() {
    for alignment in [0, 3, 8192, u64::MAX] {
        assert!(Layout::new(1, 8, alignment, 0, 65536).is_none());
    }
    assert!(Layout::new(9, 8, 8, 0, 65536).is_none());
    assert!(Layout::new(0, 0, 8, 0, 65536).is_none());
    assert!(Layout::new(0, usize::MAX, 8, 0, usize::MAX).is_none());
    assert!(Layout::new(0, 65536, 4096, 0, 65536).is_none());
    assert!(Layout::new(0, 8, 8, 0, 4095).is_none());
    assert!(Layout::new(0, 65528, 8, 0, 65536).is_some());
}
