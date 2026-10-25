use std::env;
use std::fs;
use std::path::PathBuf;

use ed25519_compact::{KeyPair, Seed};
use sha2::{Digest, Sha256};

const BOOT_MANIFEST_MAGIC: &[u8; 4] = b"WBOT";
const BOOT_MANIFEST_VERSION: u16 = 1;
const BOOT_MANIFEST_HEADER_LEN: u16 = 4 + 2 + 2 + 8 + 32 + 64;
#[allow(dead_code)]
const KERNEL_LOAD_OFFSET: u64 = 0x1000_0000_0000;
const TEST_BOOT_SEED: [u8; 32] = [
    0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c, 0x44,
    0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae, 0x7f, 0x60, 0x0b,
];

fn parse_seed_hex(value: &str) -> [u8; 32] {
    let bytes = value.as_bytes();
    assert!(
        bytes.len() == 64,
        "WOVENHAT_BOOT_SIGNING_SEED_HEX must be 64 hex digits"
    );
    let mut seed = [0u8; 32];
    let nibble = |byte: u8| match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        _ => panic!("invalid boot signing seed"),
    };
    for index in 0..32 {
        let high = nibble(bytes[index * 2]);
        let low = nibble(bytes[index * 2 + 1]);
        seed[index] = (high << 4) | low;
    }
    seed
}

fn parse_public_key_hex(value: &str) -> [u8; 32] {
    parse_seed_hex(value)
}

#[allow(dead_code, clippy::manual_is_multiple_of)]
fn loaded_kernel_image(bytes: &[u8]) -> Vec<u8> {
    assert!(
        bytes.len() >= 64 && &bytes[..4] == b"\x7fELF",
        "kernel is not ELF64"
    );
    assert!(
        bytes[4] == 2 && bytes[5] == 1,
        "kernel ELF must be little-endian ELF64"
    );
    let read_u16 = |offset: usize| u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
    let read_u64 = |offset: usize| {
        u64::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ])
    };
    let ph_offset = usize::try_from(read_u64(32)).expect("program-header offset overflow");
    let ph_size = usize::from(read_u16(54));
    let ph_count = usize::from(read_u16(56));
    assert!(ph_size >= 56, "kernel ELF program header is too small");
    let mut image_len = 0usize;
    for index in 0..ph_count {
        let offset = ph_offset
            .checked_add(index.checked_mul(ph_size).expect("program-header overflow"))
            .expect("program-header range overflow");
        assert!(
            offset + 56 <= bytes.len(),
            "kernel ELF program header is truncated"
        );
        if read_u32(bytes, offset) != 1 {
            continue;
        }
        let virtual_address =
            usize::try_from(read_u64(offset + 16)).expect("virtual address overflow");
        let memory_size = usize::try_from(read_u64(offset + 40)).expect("segment size overflow");
        image_len = image_len.max(
            virtual_address
                .checked_add(memory_size)
                .expect("kernel image size overflow"),
        );
    }
    assert!(
        image_len > 0 && image_len <= 64 * 1024 * 1024,
        "kernel image is out of bounds"
    );
    let mut loaded = vec![0u8; image_len];
    for index in 0..ph_count {
        let offset = ph_offset
            .checked_add(index.checked_mul(ph_size).expect("program-header overflow"))
            .expect("program-header range overflow");
        if read_u32(bytes, offset) != 1 {
            continue;
        }
        let file_offset = usize::try_from(read_u64(offset + 8)).expect("segment offset overflow");
        let virtual_address =
            usize::try_from(read_u64(offset + 16)).expect("virtual address overflow");
        let file_size = usize::try_from(read_u64(offset + 32)).expect("segment size overflow");
        assert!(file_offset + file_size <= bytes.len());
        assert!(virtual_address + file_size <= loaded.len());
        loaded[virtual_address..virtual_address + file_size]
            .copy_from_slice(&bytes[file_offset..file_offset + file_size]);
    }
    let mut signed_len = 0usize;
    for index in 0..ph_count {
        let offset = ph_offset
            .checked_add(index.checked_mul(ph_size).expect("program-header overflow"))
            .expect("program-header range overflow");
        if read_u32(bytes, offset) == 1 && read_u32(bytes, offset + 4) & 2 == 0 {
            let virtual_address =
                usize::try_from(read_u64(offset + 16)).expect("virtual address overflow");
            let memory_size =
                usize::try_from(read_u64(offset + 40)).expect("segment size overflow");
            signed_len = signed_len.max(
                virtual_address
                    .checked_add(memory_size)
                    .expect("signed image overflow"),
            );
        }
    }
    assert!(signed_len > 0, "kernel has no immutable load segments");
    let mut rela_address = None;
    let mut rela_size = 0usize;
    let mut rela_entry_size = 0usize;
    for index in 0..ph_count {
        let offset = ph_offset
            .checked_add(index.checked_mul(ph_size).expect("program-header overflow"))
            .expect("program-header range overflow");
        if read_u32(bytes, offset) != 2 {
            continue;
        }
        let file_offset = usize::try_from(read_u64(offset + 8)).expect("dynamic offset overflow");
        let file_size = usize::try_from(read_u64(offset + 32)).expect("dynamic size overflow");
        assert!(
            file_offset + file_size <= bytes.len(),
            "dynamic segment is truncated"
        );
        for dynamic_offset in (file_offset..file_offset + file_size).step_by(16) {
            match read_u64(dynamic_offset) {
                0 => break,
                7 => {
                    rela_address = Some(
                        usize::try_from(read_u64(dynamic_offset + 8))
                            .expect("rela address overflow"),
                    )
                }
                8 => {
                    rela_size =
                        usize::try_from(read_u64(dynamic_offset + 8)).expect("rela size overflow")
                }
                9 => {
                    rela_entry_size = usize::try_from(read_u64(dynamic_offset + 8))
                        .expect("rela entry size overflow")
                }
                _ => {}
            }
        }
    }
    if let Some(rela_address) = rela_address {
        assert!(rela_entry_size == 24 && rela_size % rela_entry_size == 0);
        for relocation in (0..rela_size).step_by(rela_entry_size) {
            let offset = rela_address + relocation;
            let target =
                usize::try_from(read_u64_from(&loaded, offset)).expect("rela target overflow");
            let info = read_u64_from(&loaded, offset + 8);
            let addend = read_u64_from(&loaded, offset + 16);
            assert!(
                info & 0xffff_ffff == 8 && info >> 32 == 0,
                "unsupported kernel relocation"
            );
            if target + 8 <= signed_len {
                let value = KERNEL_LOAD_OFFSET
                    .checked_add(addend)
                    .expect("relocation value overflow");
                loaded[target..target + 8].copy_from_slice(&value.to_le_bytes());
            }
        }
    }
    loaded.truncate(signed_len);
    loaded
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

#[allow(dead_code)]
fn read_u64_from(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(
        bytes[offset..offset + 8]
            .try_into()
            .expect("u64 out of bounds"),
    )
}

fn generate_wallpaper(out_dir: &PathBuf) -> PathBuf {
    const WIDTH: usize = 160;
    const HEIGHT: usize = 90;
    const ROW_BYTES: usize = (WIDTH * 3 + 3) & !3;
    let mut bytes = vec![0u8; 54 + ROW_BYTES * HEIGHT];
    bytes[..2].copy_from_slice(b"BM");
    bytes[2..6].copy_from_slice(&(bytes.len() as u32).to_le_bytes());
    bytes[10..14].copy_from_slice(&54u32.to_le_bytes());
    bytes[14..18].copy_from_slice(&40u32.to_le_bytes());
    bytes[18..22].copy_from_slice(&(WIDTH as i32).to_le_bytes());
    bytes[22..26].copy_from_slice(&(HEIGHT as i32).to_le_bytes());
    bytes[26..28].copy_from_slice(&1u16.to_le_bytes());
    bytes[28..30].copy_from_slice(&24u16.to_le_bytes());
    bytes[34..38].copy_from_slice(&((ROW_BYTES * HEIGHT) as u32).to_le_bytes());
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let t = (x * 255 / WIDTH) as u8;
            let row = HEIGHT - 1 - y;
            let offset = 54 + row * ROW_BYTES + x * 3;
            bytes[offset] = 80u8.saturating_add(t / 3);
            bytes[offset + 1] = 40u8.saturating_add(t / 2);
            bytes[offset + 2] = 18u8.saturating_add(t);
        }
    }
    let path = out_dir.join("wovenhat-wallpaper.bmp");
    fs::write(&path, bytes).expect("failed to write generated wallpaper");
    path
}

fn canonical_kernel_image(bytes: &[u8]) -> Vec<u8> {
    assert!(bytes.len() >= 64 && &bytes[..4] == b"\x7fELF" && bytes[4] == 2 && bytes[5] == 1);
    let read_u16 = |offset: usize| u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
    let read_u64 =
        |offset: usize| u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
    let ph_offset = usize::try_from(read_u64(32)).unwrap();
    let ph_size = usize::from(read_u16(54));
    let ph_count = usize::from(read_u16(56));
    assert!(ph_size >= 56);
    let mut canonical = Vec::new();
    for index in 0..ph_count {
        let offset = ph_offset + index.checked_mul(ph_size).unwrap();
        assert!(offset + 56 <= bytes.len());
        let flags = read_u32(bytes, offset + 4);
        if read_u32(bytes, offset) != 1 || flags & 4 == 0 || flags & 2 != 0 {
            continue;
        }
        let file_offset = usize::try_from(read_u64(offset + 8)).unwrap();
        let file_size = usize::try_from(read_u64(offset + 32)).unwrap();
        let memory_size = usize::try_from(read_u64(offset + 40)).unwrap();
        assert!(
            file_size <= memory_size && file_offset.checked_add(file_size).unwrap() <= bytes.len()
        );
        assert!(canonical.len() + memory_size <= 64 * 1024 * 1024);
        canonical.extend_from_slice(&bytes[file_offset..file_offset + file_size]);
        canonical.resize(canonical.len() + memory_size - file_size, 0);
    }
    assert!(!canonical.is_empty());
    canonical
}

#[allow(clippy::ptr_arg)]
fn create_boot_manifest(kernel: &PathBuf, out_dir: &PathBuf) -> PathBuf {
    let kernel_bytes = fs::read(kernel).expect("failed to read kernel for boot manifest");
    let loaded = canonical_kernel_image(&kernel_bytes);
    let digest: [u8; 32] = Sha256::digest(&loaded).into();
    let seed = if env::var_os("CARGO_FEATURE_STAGE15_5_TEST").is_some() {
        TEST_BOOT_SEED
    } else {
        let value = env::var("WOVENHAT_BOOT_SIGNING_SEED_HEX")
            .expect("secure-boot requires WOVENHAT_BOOT_SIGNING_SEED_HEX");
        parse_seed_hex(&value)
    };
    let key_pair = KeyPair::from_seed(Seed::new(seed));
    if env::var_os("CARGO_FEATURE_STAGE15_5_TEST").is_none() {
        let trusted_key = parse_public_key_hex(
            &env::var("WOVENHAT_BOOT_TRUSTED_PUBLIC_KEY_HEX")
                .expect("secure-boot requires WOVENHAT_BOOT_TRUSTED_PUBLIC_KEY_HEX"),
        );
        assert_eq!(
            key_pair.pk.as_ref(),
            &trusted_key,
            "boot signing seed/public key mismatch"
        );
    }
    let signature = key_pair.sk.sign(digest, None);
    let mut manifest = Vec::with_capacity(BOOT_MANIFEST_HEADER_LEN as usize);
    manifest.extend_from_slice(BOOT_MANIFEST_MAGIC);
    manifest.extend_from_slice(&BOOT_MANIFEST_VERSION.to_le_bytes());
    manifest.extend_from_slice(&BOOT_MANIFEST_HEADER_LEN.to_le_bytes());
    manifest.extend_from_slice(&(loaded.len() as u64).to_le_bytes());
    manifest.extend_from_slice(&digest);
    manifest.extend_from_slice(&*signature);
    let path = out_dir.join("wovenhat-boot-manifest.bin");
    fs::write(&path, manifest).expect("failed to write boot manifest");
    path
}

fn main() {
    let kernel = PathBuf::from(
        env::var_os("CARGO_BIN_FILE_KERNEL_wovenhat-kernel")
            .expect("WovenHat kernel artifact not found"),
    );

    // Acceptance logs and other workspace files are not image inputs. Without
    // explicit inputs Cargo scans the whole package and can rebuild the image
    // on every stress boot. Track the actual artifact so kernel changes still
    // always regenerate the boot image.
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", kernel.display());
    println!("cargo:rerun-if-env-changed=WOVENHAT_BOOT_SIGNING_SEED_HEX");
    println!("cargo:rerun-if-env-changed=WOVENHAT_BOOT_TRUSTED_PUBLIC_KEY_HEX");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR missing"));

    println!("cargo:rerun-if-changed=assets/wallpapers/manifest.toml");
    let wallpaper = generate_wallpaper(&out_dir);
    println!(
        "cargo:rustc-env=WOVENHAT_WALLPAPER_IMAGE={}",
        wallpaper.display()
    );

    let uefi_path = out_dir.join("wovenhat-os-uefi.img");

    let mut boot = bootloader::UefiBoot::new(&kernel);
    if env::var_os("CARGO_FEATURE_SECURE_BOOT").is_some() {
        let manifest = create_boot_manifest(&kernel, &out_dir);
        boot.set_ramdisk(&manifest);
    }
    boot.create_disk_image(&uefi_path)
        .expect("failed to create WovenHat OS UEFI image");

    println!(
        "cargo:rustc-env=WOVENHAT_UEFI_IMAGE={}",
        uefi_path.display()
    );
}
