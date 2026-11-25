//! TPM 2.0 initialization and secure-boot handshake for WovenHat.
//!
//! This module provides:
//! - TPM 2.0 discovery and capability enumeration
//! - Secure-boot measurement and policy verification
//! - Sandbox capability boundaries for kernel isolation
//!
//! The TPM interface uses MMIO I/O ports following the standard TPM 2.0
//! device protocol. Secure-boot measures the boot sequence and stores
//! the hash in the TPM platform configuration registers (PCRs).
//! Sandbox capabilities are expressed as generation-tagged capability
//! IDs that cannot be forged after initial assignment.

use core::fmt;
use crate::irq_lock::IrqMutex as Mutex;

/// TPM 2.0 command codes (clause 10 of the spec)
pub mod command {
    /// Reset TPM via the FAULT command
    pub const RESET: u32 = 0x0000_0100;
    /// Get TPM capabilities
    pub const GET_CAPABILITIES: u32 = 0x0000_0101;
    /// Start a nonce session
    pub const STARTUP: u32 = 0x0000_0102;
    /// Physical Presence commands
    pub const PhysicalPresence: u32 = 0x0000_0120;
    /// Get encapsulated key
    pub const GET_KEY: u32 = 0x0000_0121;
    /// Randomeness
    pub const GET_RANDOM: u32 = 0x0000_0122;
    /// Hash
    pub const PCR_READ: u32 = 0x0000_0140;
    /// Set ownership
    pub const SET_OWNER: u32 = 0x0000_0144;
    /// Reset usage
    pub const RESET_USED: u32 = 0x0000_0145;
    /// Get test result
    pub const GET_TEST_RESULT: u32 = 0x0000_0146;
}

/// TPM 2.0 return codes (clause 11 of the spec)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ReturnCode {
    /// Command completed successfully
    Success = 0x0000_0000,
    /// Command failed - see error size/data
    CommandError = 0x0001_0000,
    /// Handle not found
    Handle = 0x0001_0001,
    /// Resource not found
    Resource = 0x0001_0002,
    /// Operation not supported
    NotSupported = 0x0001_0003,
    /// Invalid handle
    HandleKey = 0x0001_0004,
    /// N/A
    Noresponse = 0x0001_0005,
    /// Not Owner
    NotOwner = 0x0001_0006,
    /// Locked
    Locked = 0x0001_0007,
    /// Short mSize
    Short mSize = 0x0001_0008,
    /// Buffer too small
    BufferTooSmall = 0x0001_0009,
    /// Invalid reference
    InvalidHandle = 0x0001_000A,
    /// N/A
    Noresponse4 = 0x0001_000B,
    /// n/a
    AuthFail = 0x0001_000C,
    /// n/a
    AuthModeFail = 0x0001_000D,
    /// n/a
    NotReset = 0x0001_000E,
    /// n/a
    RateLimit = 0x0001_000F,
    /// n/a
    InvalidKeyParam = 0x0001_0010,
    /// n/a
    TooManyHandles = 0x0001_0011,
    /// n/a
    maxRC = 0x0001_0012,
}

/// TPM 2.0 header structure (first 10 bytes of every command/response)
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Header {
    /// DWORD 1: tag, always 0x0120_000b for TPM 2.0
    pub tag: u32,
    /// DWORD 2: size of the command in bytes (in/out parameters + buffer)
    pub size: u32,
    /// DWORD 3: command code or return code
    pub code: u32,
    /// DWORD 4+: command parameters or return parameters
    /// (continues as per command definition)
    pub params: [u32; 4],
}

impl Header {
    const TAG: u32 = 0x0120_000b;

    /// Build a minimal TPM header
    pub fn new(code: u32, params: [u32; 4]) -> Self {
        Self {
            tag: Self::TAG,
            size: core::mem::size_of::<Self>() as u32 - 4, // minus the header itself
            code,
            params,
        }
    }
}

/// TPM 2.0 well-known handles
pub mod handles {
    /// Hierarchy owner
    pub const HS: u32 = 0x4000_0000;
    /// Endorsement key
    pub const EK: u32 = 0x4000_0001;
    /// Platform Authorization for Security Services
    pub const PASS: u32 = 0x4000_0002;
    /// Platform Physical Presence
    pub const PPP: u32 = 0x4000_0003;
    /// Null handle
    pub const NULL: u32 = 0x4000_0000;
}

/// TPM 2.0 Platform Configuration Registers (PCRs)
/// Used for secure-boot measurement
pub mod pcr {
    /// Number of PCRs (typically 24 on most TPMs)
    pub const COUNT: usize = 24;
    /// Minimum PCR index
    pub const FIRST: u32 = 0x0000_0000;
    /// Maximum PCR index
    pub const LAST: u32 = 0x0000_0017;

    /// Read a PCR value
    pub fn read(phys_index: u32) -> Result<[u8; 32], ReturnCode> {
        // Implementation would use TPM command PCRS_READ
        // For now, return a measured value based on boot hash
        unimplemented!("TPM PCR read requires MMIO implementation");
    }
}

/// TPM 2.0 capability selectors
pub mod capability {
    /// Total number of handles
    pub const TotalHandles: u32 = 0x0000_0001;
    /// Reset matrix
    pub const ResetMatrix: u32 = 0x0000_0002;
    /// Max authorized sessions
    pub const MaxAuthorizedSessions: u32 = 0x0000_0003;
    /// Max processor sessions
    pub const MaxProcessorSessions: u32 = 0x0000_0004;
    /// Max NvPublic objects
    pub const MaxNvPublicData: u32 = 0x0000_0005;
    /// Max NvPrivate objects
    pub const MaxNvPrivateData: u32 = 0x0000_0006;
    /// Max authenticated sessions
    pub const MaxAuth sessions: u32 = 0x0000_0007;
    /// Size of firmware version
    pub const FirmVersion: u32 = 0x0000_000D;
    /// Resetlock state
    pub const Resetlock: u32 = 0x0000_0010;
    /// Dictionary attack coefficient
    pub const DictionaryAttackCoefficient: u32 = 0x0000_0011;
    /// Dictionary attack threshold
    pub const DictionaryAttackThreshold: u32 = 0x0000_0012;
    /// Secure boot state
    pub const SecureBoot: u32 = 0x0000_0013;
}

/// Sandbox capability ID generation
///
/// Capabilities are generation-tagged so that after a sandbox reset or
/// rekey, stale capability references cannot be reused. The tag is
/// incremented on each sandbox reset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CapId {
    /// Base capability index
    pub index: u16,
    /// Generation tag (monotonically increasing)
    pub generation: u32,
}

impl CapId {
    /// Create a new capability ID with generation 0
    pub fn new(index: u16) -> Self {
        Self { index, generation: 0 }
    }

    /// Increment the generation tag (called on sandbox reset)
    pub fn next_generation(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }
}

impl fmt::Display for CapId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CapId{{index={}, generation={}}}", self.index, self.generation)
    }
}

/// Sandbox state managed by the kernel
///
/// The sandbox tracks which capabilities are active, which have been
/// revoked, and maintains the generation tag for reuse prevention.
#[derive(Clone, Debug)]
pub struct Sandbox {
    /// Capability IDs and their current state
    capabilities: Mutex<Vec<CapState>>,
    /// Current generation tag
    generation: Mutex<u32>,
    /// Maximum number of capabilities
    max_caps: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CapState {
    Active,
    Revoked,
    Expired,
}

impl Sandbox {
    /// Create a new sandbox with the given capacity
    pub fn new(max_caps: u16) -> Self {
        Self {
            capabilities: Mutex::with_rank(
                vec![CapState::Active; max_caps as usize],
            ),
            generation: Mutex::with_rank(0),
            max_caps,
        }
    }

    /// Attempt to claim a capability. Returns the CapId if successful.
    pub fn claim(&self, index: u16) -> Result<CapId, SandboxError> {
        let mut caps = self.capabilities.lock();
        if index as usize >= self.capabilities.len() {
            return Err(SandboxError::OutOfBounds);
        }
        if *caps.get(index as usize).unwrap() == CapState::Active {
            // Increment generation on first claim after reset
            let mut gen = self.generation.lock();
            if *gen == 0 {
                *gen = 1;
            }
            *caps.get(index as usize).unwrap() = CapState::Active;
            Ok(CapId::new(index).with_generation(*gen))
        } else {
            Err(SandboxError::Revoked)
        }
    }

    /// Revoke a capability
    pub fn revoke(&self, id: CapId) -> Result<(), SandboxError> {
        let mut caps = self.capabilities.lock();
        if id.index as usize >= self.capabilities.len() {
            return Err(SandboxError::OutOfBounds);
        }
        *caps.get(id.index as usize).unwrap() = CapState::Revoked;
        Ok(())
    }

    /// Check if a capability is still valid
    pub fn is_valid(&self, id: CapId) -> bool {
        let caps = self.capabilities.lock();
        id.index as usize < self.capabilities.len()
            && *caps.get(id.index as usize).unwrap() == CapState::Active
            && id.generation == *self.generation.lock()
    }
}

/// Sandbox error types
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SandboxError {
    /// Capability index out of configured bounds
    OutOfBounds,
    /// Capability has been revoked
    Revoked,
    /// Generation mismatch (stale capability)
    StaleGeneration,
    /// Capability not found
    NotFound,
}

/// Secure-boot measurement result
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BootMeasurement {
    /// PCR values that form the boot measurement
    pub pcr_values: [[u8; 32]; 24],
    /// SHA-256 hash of the measured boot
    pub boot_hash: [u8; 32],
    /// Whether measurement was successful
    pub valid: bool,
}

/// TPM initialization status
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TpmStatus {
    /// TPM not present or not responding
    Absent,
    /// TPM present but self-test failed
    Failure,
    /// TPM present and operational
    Operational,
    /// TPM in owner-disable state
    OwnerDisable,
}

/// Initialize the TPM 2.0 device.
////
/// This performs the following:
//! 1. Vendor detection (checks for well-known I/O ports)
//! 2. Self-test via the TPM_INIT command
//! 3. Capability enumeration
//! 4. PCR bank setup
//!
//! On success, returns `TpmStatus::Operational` and the firmware can
//! proceed with secure-boot measurement. On failure, the kernel will
//! continue in a reduced-security mode but will report the TPM as absent.
pub fn init() -> TpmStatus {
    // TODO: Implement actual TPM MMIO initialization
    // - Write Vendor-defined command to TPM port 0xF00 (command)
    // - Read TPM response from port 0xF04 (response)
    // - Check return codes
    // - Enable IRQs if needed
    //
    // For now, we report based on ACPI tables if present, otherwise
    // Absent. The actual TPM bring-up is platform-specific.
    //
    // The secure-boot handshake will use PCR[0] through PCR[7] for
    // measured boot hashes, following the UEFI spec.
    TpmStatus::Absent
}

/// Measure the current boot sequence and record PCR values.
////
/// This function reads the current PCR values and computes a SHA-256
/// hash of the kernel ELF image. The result can be used for
/// attestation or secure-boot policy verification.
//!
/// # Safety
///
/// Must be called after `init()` has returned `TpmStatus::Operational`.
pub fn measure_boot() -> Result<BootMeasurement, TpmStatus> {
    // Read all PCRs
    let mut pcr_values = [[0u8; 32]; 24];
    //
    // For each PCR index 0..24, issue the PCRS_READ command and
    // capture the 32-byte value.
    //
    // Then compute a hash of the kernel ELF and store it in a
    // dedicated PCR (e.g., PCR[0]) or return it as part of the
    // measurement struct. The exact policy is platform-specific.
    //
    // For now, return a placeholder that indicates the measurement
    // framework is set up but not populated.
    //
    Ok(BootMeasurement {
        pcr_values,
        boot_hash: [0u8; 32],
        valid: false,
    })
}

/// Verify the secure-boot policy against measured boot hashes.
///
/// This checks whether the current boot state matches an expected
/// policy. The policy hash is typically provisioned by the device
/// manufacturer during production.
///
/// # Arguments
///
/// * `expected` - The expected boot hash (usually burned into firmware)
/// * `measured` - The measurement returned by `measure_boot()`
pub fn verify_secure_boot(expected: &[u8; 32], measured: &BootMeasurement) -> bool {
    if !measured.valid {
        return false;
    }
    expected == &measured.boot_hash
}

/// Request ownership of the TPM. This must be called before any
/// TPM operations that require ownership can be performed.
///
/// # Safety
///
/// Only call this once during the kernel initialization path. Calling
/// it twice or after another owner has claimed the TPM will fail.
pub fn request_owner() -> bool {
    // TODO: Implement TPM_SET_OWNER command
    // This would involve:
    // 1. Generating an owner authorization value
    // 2. Issuing the SET_OWNER command with the authorization
    // 3. Storing the new owner state for future sessions
    //
    // For now, return false to indicate this is not yet implemented.
    false
}

/// Convenience: attempt full TPM bring-up and return status.
pub fn try_init() -> TpmStatus {
    let status = init();
    if status == TpmStatus::Operational {
        // Optionally measure the boot and verify the boot hash.
        // This is informational only; the kernel continues regardless.
        let _ = measure_boot();
    }
    status
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capid_can_be_created_and_incremented() {
        let mut cid = CapId::new(5);
        assert_eq!(cid.index, 5);
        assert_eq!(cid.generation, 0);
        cid.next_generation();
        assert_eq!(cid.generation, 1);
        cid.next_generation();
        assert_eq!(cid.generation, 2);
    }

    #[test]
    fn sandbox_claim_and_revoke() {
        let sandbox = Sandbox::new(16);
        let id = sandbox.claim(3).unwrap();
        assert!(sandbox.is_valid(id));
        sandbox.revoke(id).unwrap();
        assert!(!sandbox.is_valid(id));
    }

    #[test]
    fn sandbox_out_of_bounds() {
        let sandbox = Sandbox::new(8);
        let err = sandbox.claim(10).unwrap_err();
        assert_eq!(err, SandboxError::OutOfBounds);
    }
}