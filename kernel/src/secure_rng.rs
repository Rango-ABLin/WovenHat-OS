//! Bounded HMAC-SHA256 DRBG behind the Ring-3 randomness ABI.
//!
//! This module is a *deterministic generator*: it is safe only when seeded by
//! a validated entropy source. Boot seeding requires a health-checked hardware
//! RDRAND sequence mixed with boot timing. If that check is unavailable or
//! fails, the pool stays unseeded and every read fails closed.
//!
//! This is a virtual/software-qualified entropy boundary, not a certified
//! hardware random-number generator. Physical entropy-source review and
//! independent validation remain release gates; no caller may infer a
//! production trust claim from a successful fill.

use hmac::{Hmac, Mac};
use sha2::Sha256;
use spin::Mutex;
use x86_64::instructions::random::RdRand;
use zeroize::Zeroize;

type HmacSha256 = Hmac<Sha256>;

/// Largest single read, so a caller cannot hold the pool lock indefinitely.
pub const MAX_RNG_BYTES: usize = 4096;
const DOMAIN: &[u8] = b"WovenHat-SecureRng-v1";
const RATCHET: &[u8] = b"WovenHat-SecureRng-ratchet-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RngError {
    Unavailable,
    InvalidRequest,
}

struct Pool {
    key: [u8; 32],
    counter: u64,
    seeded: bool,
}

impl Pool {
    const fn new() -> Self {
        Self {
            key: [0; 32],
            counter: 0,
            seeded: false,
        }
    }

    fn clear(&mut self) {
        self.key.zeroize();
        self.counter = 0;
        self.seeded = false;
    }

    fn seed(&mut self, material: &[u8; 32]) {
        self.clear();
        let Ok(mut mac) = HmacSha256::new_from_slice(DOMAIN) else {
            return;
        };
        mac.update(material);
        let digest = mac.finalize().into_bytes();
        let mut key = [0u8; 32];
        key.copy_from_slice(&digest);
        self.key.copy_from_slice(&key);
        key.zeroize();
        self.counter = 1;
        self.seeded = true;
    }

    fn fill(&mut self, out: &mut [u8]) -> Result<(), RngError> {
        if out.is_empty() || out.len() > MAX_RNG_BYTES {
            return Err(RngError::InvalidRequest);
        }
        if !self.seeded {
            return Err(RngError::Unavailable);
        }
        for chunk in out.chunks_mut(32) {
            let Ok(mut mac) = HmacSha256::new_from_slice(&self.key) else {
                self.clear();
                return Err(RngError::Unavailable);
            };
            mac.update(&self.counter.to_be_bytes());
            let block = mac.finalize().into_bytes();
            chunk.copy_from_slice(&block[..chunk.len()]);

            // Ratchet the key after every block so a later state compromise
            // does not reveal earlier output.
            let Ok(mut ratchet) = HmacSha256::new_from_slice(&self.key) else {
                self.clear();
                return Err(RngError::Unavailable);
            };
            ratchet.update(RATCHET);
            ratchet.update(&block);
            let next = ratchet.finalize().into_bytes();
            self.key.copy_from_slice(&next);
            self.counter = self.counter.wrapping_add(1).max(1);
        }
        Ok(())
    }
}

static POOL: Mutex<Pool> = Mutex::new(Pool::new());

/// Fill a caller-owned buffer from the bounded kernel DRBG.
pub fn fill(out: &mut [u8]) -> Result<(), RngError> {
    POOL.lock().fill(out)
}

pub fn available() -> bool {
    POOL.lock().seeded
}

/// Seed once at boot from a health-checked RDRAND sequence and boot timing.
/// Health checks reject constant, zero and repeated samples before any key
/// material is derived. Returns false without touching the seeded state when
/// the hardware source is missing or fails.
pub fn seed_from_hardware() -> bool {
    if available() {
        return true;
    }
    let Some(source) = RdRand::new() else {
        return false;
    };
    let mut samples = [0u64; 8];
    for sample in samples.iter_mut() {
        let Some(value) = source.get_u64() else {
            samples.zeroize();
            return false;
        };
        if value == 0 {
            samples.zeroize();
            return false;
        }
        *sample = value;
    }
    for (index, first) in samples.iter().enumerate() {
        if samples[index + 1..].contains(first) {
            samples.zeroize();
            return false;
        }
    }

    let mut material = [0u8; 32];
    for (index, sample) in samples.iter().enumerate() {
        let offset = index * 8;
        material[offset..offset + 8].copy_from_slice(&sample.to_le_bytes());
    }
    let ticks = crate::timer::ticks();
    let Ok(mut mac) = HmacSha256::new_from_slice(DOMAIN) else {
        material.zeroize();
        samples.zeroize();
        return false;
    };
    mac.update(&material);
    mac.update(&ticks.to_le_bytes());
    let digest = mac.finalize().into_bytes();
    let mut derived = [0u8; 32];
    derived.copy_from_slice(&digest);
    POOL.lock().seed(&derived);
    derived.zeroize();

    material.zeroize();
    samples.zeroize();
    available()
}

#[cfg(feature = "secure-rng-test")]
pub fn clear_for_test() {
    POOL.lock().clear();
}

#[cfg(feature = "secure-rng-test")]
pub fn seed_for_test(seed: [u8; 32]) {
    POOL.lock().seed(&seed);
}

/// Bounded self-test of the generator, independent of the hardware source.
#[cfg(feature = "secure-rng-test")]
pub fn self_test() -> bool {
    clear_for_test();
    let mut before = [0u8; 32];
    if fill(&mut before) != Err(RngError::Unavailable) || available() {
        return false;
    }
    seed_for_test([0x5a; 32]);
    if !available() {
        return false;
    }
    let mut first = [0u8; 64];
    let mut second = [0u8; 64];
    if fill(&mut first).is_err() || fill(&mut second).is_err() {
        return false;
    }
    if first == [0; 64] || second == [0; 64] || first == second {
        return false;
    }
    let mut oversized = [0u8; MAX_RNG_BYTES + 1];
    if fill(&mut oversized) != Err(RngError::InvalidRequest) {
        return false;
    }
    if fill(&mut []) != Err(RngError::InvalidRequest) {
        return false;
    }
    clear_for_test();
    !available() && fill(&mut first) == Err(RngError::Unavailable)
}
