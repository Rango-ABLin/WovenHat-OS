use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use ed25519_compact::{KeyPair, PublicKey, Seed, Signature};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

pub struct SigningRequest {
    pub purpose: &'static str,
    pub seed_env: &'static str,
    pub trusted_public_env: &'static str,
    pub external_signer_env: &'static str,
    pub production: bool,
    pub development_seed: Option<[u8; 32]>,
}

struct TemporaryDirectory(PathBuf);

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl SigningRequest {
    pub fn sign(&self, message: &[u8]) -> Result<[u8; 64], String> {
        let seed = env::var_os(self.seed_env);
        let signer = env::var_os(self.external_signer_env);
        if seed.is_some() && signer.is_some() {
            return Err(format!(
                "{} and {} cannot both be set",
                self.seed_env, self.external_signer_env
            ));
        }

        let trusted = env::var(self.trusted_public_env)
            .ok()
            .map(|value| parse_hex_32(&value, self.trusted_public_env))
            .transpose()?;
        if self.production && trusted.is_none() {
            return Err(format!(
                "production {} signing requires {}",
                self.purpose, self.trusted_public_env
            ));
        }

        let signature = if let Some(seed) = seed {
            let seed = seed
                .into_string()
                .map_err(|_| format!("{} is not valid UTF-8", self.seed_env))?;
            let key_pair = KeyPair::from_seed(Seed::new(parse_hex_32(&seed, self.seed_env)?));
            verify_public_key(self.purpose, &key_pair.pk, trusted)?;
            key_pair.sk.sign(message, None)
        } else if let Some(signer) = signer {
            let trusted = trusted.ok_or_else(|| {
                format!(
                    "external {} signing requires {}",
                    self.purpose, self.trusted_public_env
                )
            })?;
            let signature = external_sign(self.purpose, Path::new(&signer), message, &trusted)?;
            Signature::new(signature)
        } else if self.production {
            return Err(format!(
                "production {} signing requires {} or {}",
                self.purpose, self.seed_env, self.external_signer_env
            ));
        } else if let Some(seed) = self.development_seed {
            let key_pair = KeyPair::from_seed(Seed::new(seed));
            verify_public_key(self.purpose, &key_pair.pk, trusted)?;
            key_pair.sk.sign(message, None)
        } else {
            return Err(format!(
                "{} signing key is not provisioned: set {} or {}",
                self.purpose, self.seed_env, self.external_signer_env
            ));
        };

        if let Some(trusted) = trusted {
            PublicKey::from_slice(&trusted)
                .map_err(|_| {
                    format!(
                        "{} is not a valid Ed25519 public key",
                        self.trusted_public_env
                    )
                })?
                .verify(message, &signature)
                .map_err(|_| format!("{} signing produced an invalid signature", self.purpose))?;
        }
        Ok(*signature)
    }
}

fn verify_public_key(
    purpose: &str,
    public_key: &PublicKey,
    trusted: Option<[u8; 32]>,
) -> Result<(), String> {
    if let Some(trusted) = trusted
        && public_key.as_ref() != trusted
    {
        return Err(format!(
            "{purpose} signing key does not match the trusted public key"
        ));
    }
    Ok(())
}

fn external_sign(
    purpose: &str,
    signer: &Path,
    message: &[u8],
    trusted: &[u8; 32],
) -> Result<[u8; 64], String> {
    let directory = temporary_directory(purpose)?;
    let input = directory.0.join("message.bin");
    let output = directory.0.join("signature.bin");
    fs::write(&input, message).map_err(|error| format!("write signing input: {error}"))?;

    let trusted_hex = hex(trusted);
    let mut command = if signer
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("py"))
    {
        let mut command =
            Command::new(env::var_os("WOVENHAT_SIGNER_PYTHON").unwrap_or_else(|| "python".into()));
        command.arg(signer);
        command
    } else {
        Command::new(signer)
    };
    let status = command
        .arg("sign")
        .arg(purpose)
        .arg(&input)
        .arg(&output)
        .arg(trusted_hex)
        .status()
        .map_err(|error| {
            format!(
                "start external {purpose} signer {}: {error}",
                signer.display()
            )
        })?;
    if !status.success() {
        return Err(format!("external {purpose} signer failed with {status}"));
    }
    let signature =
        fs::read(&output).map_err(|error| format!("read external {purpose} signature: {error}"))?;
    signature
        .try_into()
        .map_err(|_| format!("external {purpose} signer must return exactly 64 bytes"))
}

fn temporary_directory(purpose: &str) -> Result<TemporaryDirectory, String> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("read system time: {error}"))?
        .as_nanos();
    let sequence = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    let path = env::temp_dir().join(format!(
        "wovenhat-{purpose}-signing-{}-{nonce}-{sequence}",
        std::process::id()
    ));
    fs::create_dir(&path).map_err(|error| format!("create signing directory: {error}"))?;
    Ok(TemporaryDirectory(path))
}

fn parse_hex_32(value: &str, name: &str) -> Result<[u8; 32], String> {
    if value.len() != 64 {
        return Err(format!("{name} must contain exactly 64 hexadecimal digits"));
    }
    let mut result = [0u8; 32];
    for (index, byte) in result.iter_mut().enumerate() {
        let start = index * 2;
        *byte = u8::from_str_radix(&value[start..start + 2], 16)
            .map_err(|_| format!("{name} contains a non-hexadecimal digit"))?;
    }
    Ok(result)
}

fn hex(bytes: &[u8; 32]) -> String {
    let mut output = String::with_capacity(64);
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}
