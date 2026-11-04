#[path = "../vendor/bootloader/uefi/src/image.rs"]
mod image;

fn put(bytes: &mut [u8], offset: usize, value: u64, size: usize) {
    bytes[offset..offset + size].copy_from_slice(&value.to_le_bytes()[..size]);
}

fn valid() -> Vec<u8> {
    let contract = bootloader_api::BootloaderConfig::new_default().serialize();
    let mut b = vec![0; 2048 + contract.len()];
    b[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    for (offset, value, size) in [
        (16, 3, 2),
        (18, 62, 2),
        (20, 1, 4),
        (24, 4096, 8),
        (32, 64, 8),
        (40, 128, 8),
        (52, 64, 2),
        (54, 56, 2),
        (56, 1, 2),
        (58, 64, 2),
        (60, 3, 2),
        (62, 1, 2),
        (64, 1, 4),
        (68, 5, 4),
        (72, 1024, 8),
        (80, 4096, 8),
        (96, 1, 8),
        (104, 1, 8),
        (112, 1, 8),
        (196, 3, 4),
        (216, 320, 8),
        (224, 20, 8),
        (256, 1, 4),
        (260, 1, 4),
        (280, 2048, 8),
    ] {
        put(&mut b, offset, value, size);
    }
    put(&mut b, 288, contract.len() as u64, 8);
    b[320..340].copy_from_slice(b"\0.bootloader-config\0");
    b[2048..].copy_from_slice(&contract);
    b
}

#[test]
fn preserves_supported_contract_and_rejects_architecture() {
    let mut b = valid();
    image::validate(&b).unwrap();
    put(&mut b, 16, 2, 2);
    image::validate(&b).unwrap();
    put(&mut b, 18, 183, 2);
    assert_eq!(
        image::validate(&b).err(),
        Some(image::Error::UnsupportedFormat)
    );
}

#[test]
fn bounds_files_before_allocation_and_rejects_truncation() {
    assert_eq!(image::validate_size(0), Err(image::Error::Empty));
    assert_eq!(
        image::validate_size(image::MAX_KERNEL_BYTES + 1),
        Err(image::Error::TooLarge)
    );
    assert!(image::validate_size(image::MAX_KERNEL_BYTES).is_ok());
    let b = valid();
    for length in 1..64 {
        assert_eq!(
            image::validate(&b[..length]).err(),
            Some(image::Error::Truncated)
        );
    }
    let mut b = b;
    put(&mut b, 32, u64::MAX, 8);
    assert_eq!(image::validate(&b).err(), Some(image::Error::Malformed));
}

#[test]
fn rejects_invalid_entry_segments_and_contracts() {
    let original = valid();
    for (offset, value, size, error) in [
        (24, 4097, 8, image::Error::Malformed),
        (104, 0, 8, image::Error::Malformed),
        (80, u64::MAX, 8, image::Error::Malformed),
        (112, 3, 8, image::Error::Malformed),
        (60, 0, 2, image::Error::MissingBootContract),
        (62, 3, 2, image::Error::Malformed),
        (256, 20, 4, image::Error::Malformed),
        (2048, 0xff, 1, image::Error::UnsupportedBootContract),
    ] {
        let mut b = original.clone();
        put(&mut b, offset, value, size);
        assert_eq!(image::validate(&b).err(), Some(error), "offset {offset}");
    }
}
