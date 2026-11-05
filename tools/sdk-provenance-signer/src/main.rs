use std::{env, fs, path::PathBuf, process};

use ed25519_compact::{KeyPair, PublicKey, Seed, Signature};
use sha2::{Digest, Sha256};

const SCHEMA: &str = "wovenhat.sdk-provenance-signature/v1";

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

fn parse_hex(value: &str, bytes: &mut [u8]) -> Result<(), String> {
    if value.len() != bytes.len() * 2 {
        return Err(format!("value must contain exactly {} hexadecimal digits", bytes.len() * 2));
    }
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|_| "value contains non-hexadecimal data".to_owned())?;
    }
    Ok(())
}

fn fail(message: impl AsRef<str>) -> ! {
    eprintln!("{}", message.as_ref());
    process::exit(2);
}

fn sha256_hex(payload: &[u8]) -> String {
    hex(Sha256::digest(payload).as_ref())
}

fn field<'a>(document: &'a str, name: &str) -> Option<&'a str> {
    let marker = format!("\"{name}\": \"");
    let start = document.find(&marker)? + marker.len();
    let end = document[start..].find('"')? + start;
    Some(&document[start..end])
}

fn sign(input: PathBuf, output: PathBuf) {
    let seed_text = env::var("WOVENHAT_SDK_SIGNING_SEED_HEX")
        .unwrap_or_else(|_| fail("WOVENHAT_SDK_SIGNING_SEED_HEX is required"));
    let trusted_text = env::var("WOVENHAT_SDK_TRUSTED_PUBLIC_KEY_HEX")
        .unwrap_or_else(|_| fail("WOVENHAT_SDK_TRUSTED_PUBLIC_KEY_HEX is required"));
    let mut seed = [0u8; 32];
    let mut trusted = [0u8; 32];
    parse_hex(&seed_text, &mut seed).unwrap_or_else(|error| fail(error));
    parse_hex(&trusted_text, &mut trusted).unwrap_or_else(|error| fail(error));
    let key_pair = KeyPair::from_seed(Seed::new(seed));
    if key_pair.pk.as_ref() != trusted {
        fail("SDK signing seed does not match the trusted public key");
    }
    let payload = fs::read(&input).unwrap_or_else(|error| fail(format!("read input: {error}")));
    let signature = key_pair.sk.sign(&payload, None);
    let document = format!(
        "{{\n  \"schema\": \"{SCHEMA}\",\n  \"algorithm\": \"Ed25519\",\n  \"payload\": \"{}\",\n  \"payload_sha256\": \"sha256:{}\",\n  \"public_key\": \"{}\",\n  \"signature\": \"{}\"\n}}\n",
        input.display(), sha256_hex(&payload), hex(key_pair.pk.as_ref()), hex(signature.as_ref()),
    );
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).unwrap_or_else(|error| fail(format!("create output directory: {error}")));
    }
    fs::write(&output, document).unwrap_or_else(|error| fail(format!("write signature: {error}")));
    println!("signed {} -> {}", input.display(), output.display());
}

fn verify(signature_path: PathBuf, input: PathBuf) {
    let trusted_text = env::var("WOVENHAT_SDK_TRUSTED_PUBLIC_KEY_HEX")
        .unwrap_or_else(|_| fail("WOVENHAT_SDK_TRUSTED_PUBLIC_KEY_HEX is required"));
    let mut trusted = [0u8; 32];
    parse_hex(&trusted_text, &mut trusted).unwrap_or_else(|error| fail(error));
    let document = fs::read_to_string(&signature_path)
        .unwrap_or_else(|error| fail(format!("read signature: {error}")));
    if field(&document, "schema") != Some(SCHEMA) || field(&document, "algorithm") != Some("Ed25519") {
        fail("unsupported provenance signature envelope");
    }
    let public_key_text = field(&document, "public_key").unwrap_or_else(|| fail("signature lacks public_key"));
    let signature_text = field(&document, "signature").unwrap_or_else(|| fail("signature lacks signature"));
    let payload_hash = field(&document, "payload_sha256").unwrap_or_else(|| fail("signature lacks payload_sha256"));
    let mut public_key = [0u8; 32];
    let mut signature_bytes = [0u8; 64];
    parse_hex(public_key_text, &mut public_key).unwrap_or_else(|error| fail(error));
    parse_hex(signature_text, &mut signature_bytes).unwrap_or_else(|error| fail(error));
    if public_key != trusted {
        fail("signature public key is not the trusted production key");
    }
    let payload = fs::read(&input).unwrap_or_else(|error| fail(format!("read input: {error}")));
    if payload_hash != format!("sha256:{}", sha256_hex(&payload)) {
        fail("signed payload hash does not match input");
    }
    PublicKey::from_slice(&public_key)
        .unwrap_or_else(|error| fail(format!("invalid public key: {error}")))
        .verify(&payload, &Signature::from_slice(&signature_bytes).unwrap_or_else(|error| fail(format!("invalid signature: {error}"))))
        .unwrap_or_else(|error| fail(format!("signature verification failed: {error}")));
    println!("verified {}", input.display());
}

fn main() {
    let mut args = env::args_os();
    let _program = args.next();
    match (args.next().and_then(|value| value.into_string().ok()), args.next(), args.next()) {
        (Some(command), Some(first), Some(second)) if command == "sign" => sign(first.into(), second.into()),
        (Some(command), Some(first), Some(second)) if command == "verify" => verify(first.into(), second.into()),
        _ => fail("usage: signer sign INPUT OUTPUT | signer verify SIGNATURE INPUT"),
    }
}
