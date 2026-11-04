//! Build disposable configuration fixtures with the production image builder.

use std::{env, fs, path::PathBuf};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1);
    let kernel = PathBuf::from(args.next().ok_or("missing kernel path")?);
    let output = PathBuf::from(args.next().ok_or("missing output directory")?);
    if args.next().is_some() {
        return Err("usage: wovenhat-boot-config-fixtures <kernel> <new-output-directory>".into());
    }
    let metadata = fs::symlink_metadata(&kernel)?;
    if !metadata.is_file() || metadata.len() > 256 * 1024 * 1024 {
        return Err("kernel must be a bounded regular ELF file".into());
    }
    let bytes = fs::read(&kernel)?;
    if !bytes.starts_with(b"\x7fELF") {
        return Err("kernel is not ELF".into());
    }
    // Never overwrite an existing fixture or source image. Every image below
    // is created under this newly-created directory, with no device access.
    fs::create_dir(&output)?;
    let cases: [(&str, Option<&[u8]>); 8] = [
        ("default", None),
        ("legacy", Some(b"{}")),
        ("version1", Some(br#"{"version":1,"serial_logging":true}"#)),
        ("malformed", Some(b"{")),
        ("empty", Some(b"")),
        ("trailing", Some(b"{}garbage")),
        ("unsupported", Some(br#"{"version":2}"#)),
        ("unknown", Some(br#"{"recovery_mode":true}"#)),
    ];
    for (name, configuration) in cases {
        let mut builder = bootloader::DiskImageBuilder::new(kernel.clone());
        if let Some(configuration) = configuration {
            builder.set_file_contents("boot.json".into(), configuration.to_vec());
        }
        builder.create_uefi_image(&output.join(format!("{name}.img")))?;
        println!("Created {name}.img");
    }
    let mut oversized = bootloader::DiskImageBuilder::new(kernel);
    oversized.set_file_contents("boot.json".into(), vec![b' '; 4097]);
    oversized.create_uefi_image(&output.join("oversized.img"))?;
    println!("Created oversized.img");
    let mut wrong_arch = bytes[..64].to_vec();
    wrong_arch[18..20].copy_from_slice(&183u16.to_le_bytes());
    let mut bad_table = bytes[..64].to_vec();
    bad_table[32..40].copy_from_slice(&u64::MAX.to_le_bytes());
    for (name, payload) in [
        ("kernel-empty", Vec::new()),
        ("kernel-truncated", bytes[..32].to_vec()),
        ("kernel-corrupt", vec![0xff; 64]),
        ("kernel-wrong-arch", wrong_arch),
        ("kernel-bad-table", bad_table),
    ] {
        let mut builder = bootloader::DiskImageBuilder::empty();
        builder.set_file_contents("kernel-x86_64".into(), payload);
        builder.create_uefi_image(&output.join(format!("{name}.img")))?;
        println!("Created {name}.img");
    }
    bootloader::DiskImageBuilder::empty().create_uefi_image(&output.join("kernel-missing.img"))?;

    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("boot configuration fixtures: {error}");
        std::process::exit(1);
    }
}
