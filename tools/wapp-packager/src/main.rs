use std::{env, fs, path::PathBuf, process};

use wovenhat_release_signing::SigningRequest;

const MAGIC: &[u8; 4] = b"WAPP";
const VERSION: u16 = 1;
const HEADER_LEN: u16 = 4 + 2 + 2 + 8 + 64;
const TEST_SEED: [u8; 32] = [
    0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c, 0xc4,
    0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae, 0x7f, 0x60,
];
const RELEASE_MODE: &str = "WOVENHAT_APP_RELEASE_MODE";

fn main() {
    let mut args = env::args_os();
    let _program = args.next();
    let Some(input) = args.next().map(PathBuf::from) else {
        eprintln!("usage: wovenhat-wapp-packager INPUT OUTPUT");
        process::exit(2);
    };
    let Some(output) = args.next().map(PathBuf::from) else {
        eprintln!("usage: wovenhat-wapp-packager INPUT OUTPUT");
        process::exit(2);
    };
    if args.next().is_some() {
        eprintln!("usage: wovenhat-wapp-packager INPUT OUTPUT");
        process::exit(2);
    }

    let image = fs::read(&input).expect("failed to read application ELF");
    let release_mode = env::var(RELEASE_MODE)
        .map(|value| value == "1")
        .unwrap_or(false);
    let signature = SigningRequest {
        purpose: "app",
        seed_env: "WOVENHAT_APP_SIGNING_SEED_HEX",
        trusted_public_env: "WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX",
        external_signer_env: "WOVENHAT_APP_EXTERNAL_SIGNER",
        production: release_mode,
        development_seed: Some(TEST_SEED),
    }
    .sign(&image)
    .unwrap_or_else(|error| {
        eprintln!("{error}");
        process::exit(2);
    });
    let mut package = Vec::with_capacity(HEADER_LEN as usize + image.len());
    package.extend_from_slice(MAGIC);
    package.extend_from_slice(&VERSION.to_le_bytes());
    package.extend_from_slice(&HEADER_LEN.to_le_bytes());
    package.extend_from_slice(&(image.len() as u64).to_le_bytes());
    package.extend_from_slice(&signature);
    package.extend_from_slice(&image);
    fs::write(&output, &package).expect("failed to write WAPP package");
    println!(
        "signed {} -> {} ({} bytes)",
        input.display(),
        output.display(),
        package.len()
    );
}
