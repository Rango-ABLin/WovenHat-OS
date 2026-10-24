use std::{env, fs, path::PathBuf, process};

use ed25519_compact::{KeyPair, Seed};

const MAGIC: &[u8; 4] = b"TCB1";
const VERSION: u16 = 1;
const SIGNED_BYTES: usize = 118;
const RECORD_BYTES: usize = SIGNED_BYTES + 64;
const CREDENTIAL_ROOT_NV_INDEX: u32 = 0x0150_0000;

fn fail(message: impl AsRef<str>) -> ! {
    eprintln!("tpm-broker-config: {}", message.as_ref());
    process::exit(2);
}

fn parse_hex<const N: usize>(label: &str, value: &str) -> [u8; N] {
    if value.len() != N * 2 {
        fail(format!("{label} must contain {} hexadecimal digits", N * 2));
    }
    let mut output = [0_u8; N];
    for (index, byte) in output.iter_mut().enumerate() {
        let high = value.as_bytes()[index * 2];
        let low = value.as_bytes()[index * 2 + 1];
        let nibble = |value: u8| match value {
            b'0'..=b'9' => value - b'0',
            b'a'..=b'f' => value - b'a' + 10,
            b'A'..=b'F' => value - b'A' + 10,
            _ => fail(format!("{label} contains non-hexadecimal input")),
        };
        *byte = (nibble(high) << 4) | nibble(low);
    }
    output
}

fn parse_u32(label: &str, value: &str) -> u32 {
    let value = value.strip_prefix("0x").unwrap_or(value);
    u32::from_str_radix(value, 16).unwrap_or_else(|_| fail(format!("invalid {label}: {value}")))
}

fn parse_scheme(value: &str) -> u8 {
    match value {
        "rsa-ssa" => 0,
        "rsa-pss" => 1,
        "ecdsa" => 2,
        _ => fail("scheme must be rsa-ssa, rsa-pss, or ecdsa"),
    }
}

fn main() {
    let mut args = env::args_os();
    let _program = args.next();
    let output = args.next().map(PathBuf::from).unwrap_or_else(|| {
        fail("usage: wovenhat-tpm-broker-config OUTPUT NV_INDEX SIGNING_HANDLE SCHEME PCR DIGEST_HEX ATTESTATION_NAME_HEX NV_POLICY_HEX")
    });
    let nv_index = args
        .next()
        .map(|value| parse_u32("NV index", &value.to_string_lossy()))
        .unwrap_or_else(|| fail("missing NV index"));
    let signing_handle = args
        .next()
        .map(|value| parse_u32("signing handle", &value.to_string_lossy()))
        .unwrap_or_else(|| fail("missing signing handle"));
    let scheme = args
        .next()
        .map(|value| parse_scheme(&value.to_string_lossy()))
        .unwrap_or_else(|| fail("missing scheme"));
    let pcr = args
        .next()
        .map(|value| value.to_string_lossy().parse::<u8>().unwrap_or_else(|_| fail("PCR must be 0..23")))
        .unwrap_or_else(|| fail("missing PCR"));
    let digest = args
        .next()
        .map(|value| parse_hex::<32>("expected digest", &value.to_string_lossy()))
        .unwrap_or_else(|| fail("missing expected digest"));
    let name = args
        .next()
        .map(|value| parse_hex::<34>("attestation Name", &value.to_string_lossy()))
        .unwrap_or_else(|| fail("missing attestation Name"));
    let nv_policy = args
        .next()
        .map(|value| parse_hex::<32>("NV policy", &value.to_string_lossy()))
        .unwrap_or_else(|| fail("missing NV policy"));
    if args.next().is_some() {
        fail("too many arguments");
    }
    if nv_index != CREDENTIAL_ROOT_NV_INDEX || signing_handle == 0 || pcr >= 24 {
        fail("NV index, signing handle, or PCR is outside the production bounds");
    }
    if digest.iter().all(|byte| *byte == 0)
        || name[..2] != 0x000b_u16.to_be_bytes()
        || nv_policy.iter().all(|byte| *byte == 0)
    {
        fail("digest, SHA-256 Name, and NV policy must be non-zero and valid");
    }

    let seed = env::var("WOVENHAT_TPM_POLICY_SIGNING_SEED_HEX")
        .unwrap_or_else(|_| fail("WOVENHAT_TPM_POLICY_SIGNING_SEED_HEX is required"));
    let seed = parse_hex::<32>("TPM policy signing seed", &seed);
    let key_pair = KeyPair::from_seed(Seed::new(seed));
    let trusted = env::var("WOVENHAT_TPM_POLICY_TRUSTED_PUBLIC_KEY_HEX")
        .unwrap_or_else(|_| fail("WOVENHAT_TPM_POLICY_TRUSTED_PUBLIC_KEY_HEX is required"));
    if key_pair.pk.as_ref() != parse_hex::<32>("TPM policy trusted public key", &trusted).as_ref() {
        fail("signing seed does not match the trusted public key");
    }

    let mut record = [0_u8; RECORD_BYTES];
    record[..4].copy_from_slice(MAGIC);
    record[4..6].copy_from_slice(&VERSION.to_be_bytes());
    record[6..8].copy_from_slice(&(SIGNED_BYTES as u16).to_be_bytes());
    record[8..12].copy_from_slice(&nv_index.to_be_bytes());
    record[12..16].copy_from_slice(&signing_handle.to_be_bytes());
    record[16] = scheme;
    record[17] = pcr;
    record[20..52].copy_from_slice(&digest);
    record[52..86].copy_from_slice(&name);
    record[86..118].copy_from_slice(&nv_policy);
    let signature = key_pair.sk.sign(&record[..SIGNED_BYTES], None);
    record[SIGNED_BYTES..].copy_from_slice(&*signature);
    fs::write(&output, record).unwrap_or_else(|error| fail(format!("failed to write {}: {error}", output.display())));
    println!("signed TCB1 broker policy -> {} ({} bytes)", output.display(), RECORD_BYTES);
}
