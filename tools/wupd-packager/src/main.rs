use std::{env, fs, path::PathBuf, process};

use ed25519_compact::{KeyPair, Seed};
use sha2::{Digest, Sha256};

const MAGIC: &[u8; 4] = b"WUPD";
const VERSION: u16 = 1;
const HEADER_LEN: usize = 4 + 2 + 2 + 8 + 8 + 32 + 64;
const MAX_IMAGE_BYTES: u64 = 64 * 1024 * 1024;
const RELEASE_MODE: &str = "WOVENHAT_APP_RELEASE_MODE";
const DOMAIN: &[u8] = b"WOVENHAT-UPDATE-V1";
const TEST_SEED: [u8; 32] = [
    0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c, 0xc4,
    0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae, 0x7f, 0x60,
];

fn usage() -> ! {
    eprintln!(
        "usage: wovenhat-wupd-packager INPUT OUTPUT GENERATION\n\
         production signing uses WOVENHAT_APP_SIGNING_SEED_HEX and requires\n\
         WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX"
    );
    process::exit(2)
}

fn parse_hex(value: &str) -> Result<[u8; 32], &'static str> {
    if value.len() != 64 {
        return Err("key must contain 64 hexadecimal digits");
    }
    let mut result = [0u8; 32];
    for (index, byte) in result.iter_mut().enumerate() {
        let nibble = |value: u8| match value {
            b'0'..=b'9' => Some(value - b'0'),
            b'a'..=b'f' => Some(value - b'a' + 10),
            b'A'..=b'F' => Some(value - b'A' + 10),
            _ => None,
        };
        let high = nibble(value.as_bytes()[index * 2]).ok_or("key contains non-hex digit")?;
        let low = nibble(value.as_bytes()[index * 2 + 1]).ok_or("key contains non-hex digit")?;
        *byte = (high << 4) | low;
    }
    Ok(result)
}

fn parse_generation(value: &str) -> u64 {
    let generation = value.parse::<u64>().unwrap_or_else(|_| {
        eprintln!("generation must be a positive unsigned integer");
        process::exit(2)
    });
    if generation == 0 {
        eprintln!("generation must be a positive unsigned integer");
        process::exit(2)
    }
    generation
}

fn signing_key() -> KeyPair {
    let release_mode = env::var(RELEASE_MODE)
        .map(|value| value == "1")
        .unwrap_or(false);
    let seed = match env::var("WOVENHAT_APP_SIGNING_SEED_HEX") {
        Ok(value) => parse_hex(&value).unwrap_or_else(|error| {
            eprintln!("invalid WOVENHAT_APP_SIGNING_SEED_HEX: {error}");
            process::exit(2)
        }),
        Err(_) if release_mode => {
            eprintln!("production WUPD packaging requires WOVENHAT_APP_SIGNING_SEED_HEX");
            process::exit(2);
        }
        Err(_) => TEST_SEED,
    };
    let key_pair = KeyPair::from_seed(Seed::new(seed));
    if release_mode {
        let trusted = env::var("WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX").unwrap_or_else(|_| {
            eprintln!("production WUPD packaging requires WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX");
            process::exit(2)
        });
        let trusted = parse_hex(&trusted).unwrap_or_else(|error| {
            eprintln!("invalid WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX: {error}");
            process::exit(2)
        });
        if key_pair.pk.as_ref() != trusted {
            eprintln!("application signing key mismatch");
            process::exit(2);
        }
    }
    key_pair
}

fn package(input: &[u8], generation: u64, key_pair: &KeyPair) -> Vec<u8> {
    let digest: [u8; 32] = Sha256::digest(input).into();
    let mut signed = Sha256::new();
    signed.update(DOMAIN);
    signed.update(MAGIC);
    signed.update(VERSION.to_le_bytes());
    signed.update((HEADER_LEN as u16).to_le_bytes());
    signed.update(generation.to_le_bytes());
    signed.update((input.len() as u64).to_le_bytes());
    signed.update(digest);
    signed.update(input);
    let signature = key_pair.sk.sign(signed.finalize(), None);

    let mut output = Vec::with_capacity(HEADER_LEN + input.len());
    output.extend_from_slice(MAGIC);
    output.extend_from_slice(&VERSION.to_le_bytes());
    output.extend_from_slice(&(HEADER_LEN as u16).to_le_bytes());
    output.extend_from_slice(&generation.to_le_bytes());
    output.extend_from_slice(&(input.len() as u64).to_le_bytes());
    output.extend_from_slice(&digest);
    output.extend_from_slice(&*signature);
    output.extend_from_slice(input);
    output
}

fn main() {
    let mut args = env::args_os();
    let _program = args.next();
    let Some(input) = args.next().map(PathBuf::from) else {
        usage()
    };
    let Some(output) = args.next().map(PathBuf::from) else {
        usage()
    };
    let Some(generation) = args.next().and_then(|value| value.into_string().ok()) else {
        usage()
    };
    if args.next().is_some() {
        usage();
    }
    if input == output {
        eprintln!("input and output must be different files");
        process::exit(2);
    }
    let length = fs::metadata(&input).unwrap_or_else(|error| {
        eprintln!("failed to inspect input: {error}");
        process::exit(2)
    });
    if length.len() == 0 || length.len() > MAX_IMAGE_BYTES {
        eprintln!("input image must be 1..={MAX_IMAGE_BYTES} bytes");
        process::exit(2);
    }
    let image = fs::read(&input).unwrap_or_else(|error| {
        eprintln!("failed to read input: {error}");
        process::exit(2)
    });
    let generation = parse_generation(&generation);
    let package = package(&image, generation, &signing_key());
    fs::write(&output, &package).unwrap_or_else(|error| {
        eprintln!("failed to write output: {error}");
        process::exit(2)
    });
    println!(
        "signed update generation {generation}: {} -> {} ({} bytes)",
        input.display(),
        output.display(),
        package.len()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_case_insensitively() {
        assert_eq!(parse_hex(&"Aa".repeat(32)).unwrap(), [0xaa; 32]);
        assert!(parse_hex("00").is_err());
        assert!(parse_hex(&format!("{}g", "0".repeat(63))).is_err());
    }

    #[test]
    fn package_matches_kernel_layout_and_signature_input() {
        let key_pair = KeyPair::from_seed(Seed::new(TEST_SEED));
        let package = package(b"kernel-image", 7, &key_pair);
        assert_eq!(&package[..4], MAGIC);
        assert_eq!(u16::from_le_bytes([package[4], package[5]]), VERSION);
        assert_eq!(
            u16::from_le_bytes([package[6], package[7]]) as usize,
            HEADER_LEN
        );
        assert_eq!(u64::from_le_bytes(package[8..16].try_into().unwrap()), 7);
        assert_eq!(package.len(), HEADER_LEN + b"kernel-image".len());
        assert_eq!(&package[HEADER_LEN..], b"kernel-image");
    }

    #[test]
    fn package_digest_is_image_digest() {
        let key_pair = KeyPair::from_seed(Seed::new(TEST_SEED));
        let image = b"kernel-image";
        let package = package(image, 1, &key_pair);
        let expected: [u8; 32] = Sha256::digest(image).into();
        assert_eq!(&package[24..56], &expected);
    }

    #[test]
    fn package_signature_covers_manifest_and_image() {
        let key_pair = KeyPair::from_seed(Seed::new(TEST_SEED));
        let image = b"kernel-image";
        let package = package(image, 3, &key_pair);
        let mut signed = Sha256::new();
        signed.update(DOMAIN);
        signed.update(&package[..56]);
        signed.update(image);
        let signature = ed25519_compact::Signature::from_slice(&package[56..120]).unwrap();
        assert!(key_pair.pk.verify(signed.finalize(), &signature).is_ok());
        let mut tampered = package;
        tampered[8] ^= 1;
        let mut tampered_digest = Sha256::new();
        tampered_digest.update(DOMAIN);
        tampered_digest.update(&tampered[..56]);
        tampered_digest.update(image);
        assert!(key_pair
            .pk
            .verify(tampered_digest.finalize(), &signature)
            .is_err());
    }
}
