//! Local account management for the login manager: changing a signed-in
//! account's own PIN (Stage 20.12/20.15 follow-up).
//!
//! The accounts file WovenShell reads (`/etc/wovenhat/accounts`,
//! `name:salt-hex:hash-hex` per line, see `libwoven::accounts`) is created
//! read-only on purpose: an arbitrary process with `FileWrite` must not be
//! able to rewrite the login database directly. [`change_pin`] is this
//! file's only writer, reachable only through a syscall gated on
//! `Capability::AccountManage` (the desktop session only), and only after
//! verifying the caller already knows the account's current PIN -- the PIN
//! check is this module's authorization, not a file permission bit.

use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;

pub const ACCOUNTS_PATH: &str = "/etc/wovenhat/accounts";
pub(crate) const MAX_NAME: usize = 16;
const MAX_SALT: usize = 32;
const SALT_BYTES: usize = 16;
const HASH_BYTES: usize = 32;
/// Must match `apps/woven-shell`'s `MAX_PIN`.
pub const MAX_PIN: usize = 16;
/// Must match `libwoven::accounts::PBKDF2_ITERATIONS`.
const PBKDF2_ITERATIONS: u32 = 100_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// No accounts file, or the name does not appear in it.
    NotFound,
    /// `old_pin` did not verify against the stored hash.
    WrongPin,
    /// `new_pin` is empty or longer than `MAX_PIN`.
    InvalidPin,
    /// The accounts file could not be read or written.
    Io,
}

fn hash_pin(salt: &[u8], pin: &[u8]) -> [u8; HASH_BYTES] {
    let mut out = [0u8; HASH_BYTES];
    pbkdf2_hmac::<Sha256>(pin, salt, PBKDF2_ITERATIONS, &mut out);
    out
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn decode_hex(text: &[u8], out: &mut [u8]) -> Option<usize> {
    if !text.len().is_multiple_of(2) || text.len() / 2 > out.len() {
        return None;
    }
    for (slot, pair) in out.iter_mut().zip(text.as_chunks::<2>().0) {
        *slot = hex_digit(pair[0])? << 4 | hex_digit(pair[1])?;
    }
    Some(text.len() / 2)
}

fn push_hex(line: &mut alloc::string::String, bytes: &[u8]) {
    for byte in bytes {
        line.push_str(&alloc::format!("{byte:02x}"));
    }
}

/// One parsed account line's fields, enough to verify it.
struct Line<'a> {
    name: &'a [u8],
    salt: [u8; MAX_SALT],
    salt_len: usize,
    hash: [u8; HASH_BYTES],
}

fn parse_line(line: &[u8]) -> Option<Line<'_>> {
    let line = line.strip_suffix(b"\r").unwrap_or(line);
    let mut fields = line.split(|byte| *byte == b':');
    let (name, salt_hex, hash_hex) = (fields.next()?, fields.next()?, fields.next()?);
    if fields.next().is_some() || name.is_empty() || name.len() > MAX_NAME {
        return None;
    }
    let mut salt = [0u8; MAX_SALT];
    let salt_len = decode_hex(salt_hex, &mut salt).filter(|length| *length > 0)?;
    let mut hash = [0u8; HASH_BYTES];
    if decode_hex(hash_hex, &mut hash)? != HASH_BYTES {
        return None;
    }
    Some(Line {
        name,
        salt,
        salt_len,
        hash,
    })
}

/// A fresh, unique (not secret) salt. Unlike a key or nonce, a PBKDF2 salt is
/// stored in cleartext right next to the hash it salted, so its purpose is
/// only to stop cross-account/rotation precomputation, not to resist
/// prediction -- `entropy::random_u64`'s best-effort (RDRAND when present,
/// else timer/stack-mixed) fallback, which this crate otherwise reserves for
/// non-cryptographic use, is adequate here specifically because this value
/// is never secret.
fn fresh_salt() -> [u8; SALT_BYTES] {
    let mut salt = [0u8; SALT_BYTES];
    for chunk in salt.chunks_mut(8) {
        chunk.copy_from_slice(&crate::entropy::random_u64().to_le_bytes()[..chunk.len()]);
    }
    salt
}

/// Change `name`'s PIN from `old_pin` to `new_pin` in the accounts file.
/// `old_pin` must verify against the account's current hash first; every
/// other account's line is left byte-for-byte unchanged.
pub fn change_pin(name: &[u8], old_pin: &[u8], new_pin: &[u8]) -> Result<(), Error> {
    let salt = fresh_salt();
    change_pin_with_salt(name, old_pin, new_pin, &salt)
}

/// Rewrite one account's credential with a caller-provided kernel-owned salt.
/// The salt is bounded and public; the caller uses this variant when another
/// kernel credential authority must commit the identical credential material.
pub(crate) fn change_pin_with_salt(
    name: &[u8],
    old_pin: &[u8],
    new_pin: &[u8],
    salt: &[u8],
) -> Result<(), Error> {
    if new_pin.is_empty() || new_pin.len() > MAX_PIN {
        return Err(Error::InvalidPin);
    }
    if salt.is_empty() || salt.len() > MAX_SALT {
        return Err(Error::Io);
    }
    let stat = crate::vfs::stat(ACCOUNTS_PATH).map_err(|_| Error::NotFound)?;
    if stat.kind != crate::vfs::NodeKind::File
        || stat.size == 0
        || stat.size > crate::vfs::NODE_CAPACITY
    {
        return Err(Error::Io);
    }
    let mut data = alloc::vec![0u8; stat.size];
    if crate::vfs::read_all(ACCOUNTS_PATH, &mut data) != Ok(stat.size) {
        return Err(Error::Io);
    }

    let mut found = false;
    let mut rewritten = alloc::string::String::new();
    for line in data.split(|byte| *byte == b'\n') {
        // Every line here, including the last, is written with a trailing
        // '\n' (see `kernel::main::install_hardware_demo_accounts` and
        // `libwoven`'s own writers), so splitting on it leaves one trailing
        // empty segment that is not a line at all. Skipping it here, rather
        // than re-emitting a blank line for it, is what keeps a rewrite from
        // growing the file by one blank line every time it runs.
        if line.is_empty() {
            continue;
        }
        match parse_line(line) {
            Some(parsed) if !found && parsed.name == name => {
                let candidate = hash_pin(&parsed.salt[..parsed.salt_len], old_pin);
                let mut difference = 0u8;
                for (a, b) in candidate.iter().zip(&parsed.hash) {
                    difference |= a ^ b;
                }
                if difference != 0 {
                    return Err(Error::WrongPin);
                }
                found = true;
                let new_hash = hash_pin(salt, new_pin);
                rewritten.push_str(core::str::from_utf8(name).unwrap_or(""));
                rewritten.push(':');
                push_hex(&mut rewritten, salt);
                rewritten.push(':');
                push_hex(&mut rewritten, &new_hash);
                rewritten.push('\n');
            }
            _ => {
                rewritten.push_str(core::str::from_utf8(line).unwrap_or(""));
                rewritten.push('\n');
            }
        }
    }
    if !found {
        return Err(Error::NotFound);
    }
    crate::vfs::overwrite_trusted(ACCOUNTS_PATH, rewritten.as_bytes()).map_err(|_| Error::Io)
}

/// Add an already-derived account credential to the trusted account file.
/// The password never enters this module; only bounded salt and digest bytes
/// produced by the identity authority are accepted.
#[cfg(feature = "system-apps")]
pub fn add_stored_user(name: &[u8], salt: &[u8], hash: &[u8; 32]) -> Result<(), Error> {
    if name.is_empty() || name.len() > MAX_NAME || salt.is_empty() || salt.len() > MAX_SALT {
        return Err(Error::InvalidPin);
    }
    let stat = crate::vfs::stat(ACCOUNTS_PATH).map_err(|_| Error::NotFound)?;
    if stat.kind != crate::vfs::NodeKind::File || stat.size == 0 || stat.size > crate::vfs::NODE_CAPACITY {
        return Err(Error::Io);
    }
    let mut data = alloc::vec![0u8; stat.size];
    if crate::vfs::read_all(ACCOUNTS_PATH, &mut data) != Ok(stat.size) {
        return Err(Error::Io);
    }
    let mut rewritten = alloc::string::String::new();
    for line in data.split(|byte| *byte == b'\n') {
        if line.is_empty() {
            continue;
        }
        let parsed = parse_line(line).ok_or(Error::Io)?;
        if parsed.name == name {
            return Err(Error::Io);
        }
        rewritten.push_str(core::str::from_utf8(line).map_err(|_| Error::Io)?);
        rewritten.push('\n');
    }
    rewritten.push_str(core::str::from_utf8(name).map_err(|_| Error::Io)?);
    rewritten.push(':');
    push_hex(&mut rewritten, salt);
    rewritten.push(':');
    push_hex(&mut rewritten, hash);
    rewritten.push('\n');
    if rewritten.len() > crate::vfs::NODE_CAPACITY {
        return Err(Error::Io);
    }
    crate::vfs::overwrite_trusted(ACCOUNTS_PATH, rewritten.as_bytes()).map_err(|_| Error::Io)
}

/// Remove an inactive account from the trusted account file.
#[cfg(feature = "system-apps")]
pub fn remove_user(name: &[u8]) -> Result<(), Error> {
    if name.is_empty() || name.len() > MAX_NAME {
        return Err(Error::NotFound);
    }
    let stat = crate::vfs::stat(ACCOUNTS_PATH).map_err(|_| Error::NotFound)?;
    if stat.kind != crate::vfs::NodeKind::File || stat.size == 0 || stat.size > crate::vfs::NODE_CAPACITY {
        return Err(Error::Io);
    }
    let mut data = alloc::vec![0u8; stat.size];
    if crate::vfs::read_all(ACCOUNTS_PATH, &mut data) != Ok(stat.size) {
        return Err(Error::Io);
    }
    let mut found = false;
    let mut rewritten = alloc::string::String::new();
    for line in data.split(|byte| *byte == b'\n') {
        if line.is_empty() {
            continue;
        }
        let parsed = parse_line(line).ok_or(Error::Io)?;
        if parsed.name == name {
            found = true;
            continue;
        }
        rewritten.push_str(core::str::from_utf8(line).map_err(|_| Error::Io)?);
        rewritten.push('\n');
    }
    if !found || rewritten.is_empty() {
        return Err(if found { Error::Io } else { Error::NotFound });
    }
    crate::vfs::overwrite_trusted(ACCOUNTS_PATH, rewritten.as_bytes()).map_err(|_| Error::Io)
}
