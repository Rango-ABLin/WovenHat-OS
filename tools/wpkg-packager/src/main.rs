use std::{env, fs, path::PathBuf, process};

use ed25519_compact::{KeyPair, Seed};
use libwoven::package::{HEADER_LEN, MAGIC, VERSION, parse};
use sha2::{Digest, Sha256};

const DOMAIN: &[u8] = b"WOVENHAT-WPKG-V1";
const TEST_SEED: [u8; 32] = [
    0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c, 0xc4,
    0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae, 0x7f, 0x60,
];

fn usage() -> ! {
    eprintln!(
        "usage: wovenhat-wpkg-packager INPUT_ELF OUTPUT --name NAME --version N \
         [--capabilities BITS] [--dependency NAME]... [--resources FILE]"
    );
    process::exit(2);
}

fn parse_hex(value: &str) -> [u8; 32] {
    if value.len() != 64 {
        panic!("key must contain 64 hexadecimal digits");
    }
    let mut result = [0u8; 32];
    for (index, byte) in result.iter_mut().enumerate() {
        let nibble = |value: u8| match value {
            b'0'..=b'9' => value - b'0',
            b'a'..=b'f' => value - b'a' + 10,
            b'A'..=b'F' => value - b'A' + 10,
            _ => panic!("key contains a non-hexadecimal digit"),
        };
        *byte = nibble(value.as_bytes()[index * 2]) << 4 | nibble(value.as_bytes()[index * 2 + 1]);
    }
    result
}

fn valid_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= libwoven::package::MAX_NAME_BYTES
        && value.bytes().all(
            |byte| matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'.' | b'_' | b'-'),
        )
}

fn main() {
    let mut args = env::args_os();
    let _program = args.next();
    let input = args.next().map(PathBuf::from).unwrap_or_else(|| usage());
    let output = args.next().map(PathBuf::from).unwrap_or_else(|| usage());
    let mut name = None;
    let mut version: Option<u64> = None;
    let mut capabilities = 0u64;
    let mut dependencies = Vec::new();
    let mut resources = Vec::new();
    while let Some(argument) = args.next() {
        match argument.to_str() {
            Some("--name") => {
                name = Some(
                    args.next()
                        .and_then(|value| value.into_string().ok())
                        .unwrap_or_else(|| usage()),
                )
            }
            Some("--version") => {
                version = Some(
                    args.next()
                        .and_then(|value| value.into_string().ok())
                        .and_then(|value| value.parse().ok())
                        .unwrap_or_else(|| usage()),
                )
            }
            Some("--capabilities") => {
                let value = args
                    .next()
                    .and_then(|value| value.into_string().ok())
                    .unwrap_or_else(|| usage());
                capabilities = value.strip_prefix("0x").map_or_else(
                    || value.parse().unwrap_or_else(|_| usage()),
                    |hex| u64::from_str_radix(hex, 16).unwrap_or_else(|_| usage()),
                );
            }
            Some("--dependency") => dependencies.push(
                args.next()
                    .and_then(|value| value.into_string().ok())
                    .unwrap_or_else(|| usage()),
            ),
            Some("--resources") => {
                resources.push(args.next().map(PathBuf::from).unwrap_or_else(|| usage()))
            }
            _ => usage(),
        }
    }
    let name = name.unwrap_or_else(|| usage());
    let version = version.unwrap_or_else(|| usage());
    if !valid_name(&name)
        || version == 0
        || dependencies.len() > libwoven::package::MAX_DEPENDENCIES
        || dependencies.iter().any(|value| !valid_name(value))
    {
        eprintln!("invalid package manifest");
        process::exit(2);
    }

    eprintln!("[WPKG-DIAG] read application");
    let application = fs::read(&input).expect("failed to read application ELF");
    if application.is_empty()
        || application.len() as u64 > libwoven::package::MAX_APPLICATION_BYTES
        || application.get(..4) != Some(b"\x7fELF")
    {
        eprintln!("input must be a nonempty ELF within the WPKG application limit");
        process::exit(2);
    }
    let mut resource_bytes = Vec::new();
    for path in resources {
        resource_bytes.extend(fs::read(path).expect("failed to read resource"));
    }
    if resource_bytes.len() as u64 > libwoven::package::MAX_RESOURCE_BYTES {
        eprintln!("resources exceed the WPKG limit");
        process::exit(2);
    }

    eprintln!("[WPKG-DIAG] build manifest");
    let mut manifest = Vec::new();
    manifest.push(name.len() as u8);
    manifest.extend_from_slice(name.as_bytes());
    manifest.extend_from_slice(&version.to_le_bytes());
    manifest.extend_from_slice(&capabilities.to_le_bytes());
    manifest.push(dependencies.len() as u8);
    for dependency in dependencies {
        manifest.push(dependency.len() as u8);
        manifest.extend_from_slice(dependency.as_bytes());
    }

    let mut package =
        Vec::with_capacity(HEADER_LEN + manifest.len() + application.len() + resource_bytes.len());
    package.extend_from_slice(&MAGIC);
    package.extend_from_slice(&VERSION.to_le_bytes());
    package.extend_from_slice(&(HEADER_LEN as u16).to_le_bytes());
    package.extend_from_slice(&(manifest.len() as u32).to_le_bytes());
    package.extend_from_slice(&(application.len() as u64).to_le_bytes());
    package.extend_from_slice(&(resource_bytes.len() as u64).to_le_bytes());
    package.extend_from_slice(&[0u8; 64]);
    package.extend_from_slice(&manifest);
    package.extend_from_slice(&application);
    package.extend_from_slice(&resource_bytes);

    eprintln!("[WPKG-DIAG] package assembled bytes={}", package.len());
    let seed = env::var("WOVENHAT_APP_SIGNING_SEED_HEX")
        .map(|value| parse_hex(&value))
        .unwrap_or(TEST_SEED);
    eprintln!("[WPKG-DIAG] derive signing key");
    let key_pair = KeyPair::from_seed(Seed::new(seed));
    eprintln!("[WPKG-DIAG] signing key derived");
    if let Ok(value) = env::var("WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX") {
        assert_eq!(
            key_pair.pk.as_ref(),
            &parse_hex(&value),
            "application signing key mismatch"
        );
    }
    let mut signed = Sha256::new();
    signed.update(DOMAIN);
    signed.update(&package[..28]);
    signed.update(&package[HEADER_LEN..]);
    let digest: [u8; 32] = signed.finalize().into();
    eprintln!("[WPKG-DIAG] sign digest");
    let signature = key_pair.sk.sign(digest, None);
    eprintln!("[WPKG-DIAG] digest signed");
    package[28..HEADER_LEN].copy_from_slice(signature.as_ref());
    eprintln!("[WPKG-DIAG] validate package envelope");
    assert!(
        parse(&package).is_ok(),
        "packager emitted an invalid WPKG envelope"
    );
    fs::write(&output, &package).expect("failed to write WPKG package");
    println!(
        "signed {} -> {} ({} bytes)",
        input.display(),
        output.display(),
        package.len()
    );
}
