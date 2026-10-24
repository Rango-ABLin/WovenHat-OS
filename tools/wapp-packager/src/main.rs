use std::{env, fs, path::PathBuf, process};

use ed25519_compact::{KeyPair, Seed};

const MAGIC: &[u8; 4] = b"WAPP";
const VERSION: u16 = 1;
const HEADER_LEN: u16 = 4 + 2 + 2 + 8 + 64;
const TEST_SEED: [u8; 32] = [
    0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c, 0xc4,
    0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae, 0x7f, 0x60,
];
const RELEASE_MODE: &str = "WOVENHAT_APP_RELEASE_MODE";

fn parse_hex(value: &str) -> [u8; 32] {
    assert!(value.len() == 64, "key must contain 64 hexadecimal digits");
    let mut result = [0u8; 32];
    for (index, byte) in result.iter_mut().enumerate() {
        let high = value.as_bytes()[index * 2];
        let low = value.as_bytes()[index * 2 + 1];
        let nibble = |value: u8| match value {
            b'0'..=b'9' => value - b'0',
            b'a'..=b'f' => value - b'a' + 10,
            b'A'..=b'F' => value - b'A' + 10,
            _ => panic!("key contains a non-hexadecimal digit"),
        };
        *byte = nibble(high) << 4 | nibble(low);
    }
    result
}

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
    let seed = match env::var("WOVENHAT_APP_SIGNING_SEED_HEX") {
        Ok(value) => parse_hex(&value),
        Err(_) if release_mode => {
            eprintln!("production WAPP packaging requires WOVENHAT_APP_SIGNING_SEED_HEX");
            process::exit(2);
        }
        Err(_) => TEST_SEED,
    };
    let key_pair = KeyPair::from_seed(Seed::new(seed));
    if release_mode && env::var("WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX").is_err() {
        eprintln!("production WAPP packaging requires WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX");
        process::exit(2);
    }
    if let Ok(value) = env::var("WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX") {
        assert_eq!(
            key_pair.pk.as_ref(),
            &parse_hex(&value),
            "application signing key mismatch"
        );
    }

    let signature = key_pair.sk.sign(&image, None);
    let mut package = Vec::with_capacity(HEADER_LEN as usize + image.len());
    package.extend_from_slice(MAGIC);
    package.extend_from_slice(&VERSION.to_le_bytes());
    package.extend_from_slice(&HEADER_LEN.to_le_bytes());
    package.extend_from_slice(&(image.len() as u64).to_le_bytes());
    package.extend_from_slice(&*signature);
    package.extend_from_slice(&image);
    fs::write(&output, &package).expect("failed to write WAPP package");
    println!(
        "signed {} -> {} ({} bytes)",
        input.display(),
        output.display(),
        package.len()
    );
}
