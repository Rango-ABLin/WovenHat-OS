//! Versioned native WovenHat application contract.
//!
//! This is metadata and negotiation for the public SDK boundary. It does not
//! imply Linux or Windows compatibility and it does not silently translate an
//! unsupported syscall or IPC message version.

/// Major version of the stable native application contract.
pub const ABI_MAJOR: u16 = 1;
/// Highest minor contract revision implemented by this SDK.
pub const ABI_MINOR: u16 = 0;
/// Wire encoding version for the descriptor exchanged by SDK tooling.
pub const DESCRIPTOR_VERSION: u16 = 1;
pub const DESCRIPTOR_BYTES: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Contract {
    pub major: u16,
    pub minor: u16,
    pub descriptor: u16,
}

impl Contract {
    pub const CURRENT: Self = Self {
        major: ABI_MAJOR,
        minor: ABI_MINOR,
        descriptor: DESCRIPTOR_VERSION,
    };

    pub const fn new(major: u16, minor: u16, descriptor: u16) -> Self {
        Self {
            major,
            minor,
            descriptor,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    UnsupportedMajor { requested: u16, supported: u16 },
    UnsupportedMinor { requested: u16, supported: u16 },
    UnsupportedDescriptor { requested: u16, supported: u16 },
}

/// Negotiate a contract without widening the supported ABI.
///
/// A client may request an older minor revision of the same major contract;
/// newer or unrelated major/descriptor versions are rejected explicitly.
pub const fn negotiate(requested: Contract) -> Result<Contract, Error> {
    if requested.major != ABI_MAJOR {
        return Err(Error::UnsupportedMajor {
            requested: requested.major,
            supported: ABI_MAJOR,
        });
    }
    if requested.descriptor != DESCRIPTOR_VERSION {
        return Err(Error::UnsupportedDescriptor {
            requested: requested.descriptor,
            supported: DESCRIPTOR_VERSION,
        });
    }
    if requested.minor > ABI_MINOR {
        return Err(Error::UnsupportedMinor {
            requested: requested.minor,
            supported: ABI_MINOR,
        });
    }
    Ok(Contract {
        major: ABI_MAJOR,
        minor: requested.minor,
        descriptor: DESCRIPTOR_VERSION,
    })
}

fn encode(contract: Contract, output: &mut [u8; DESCRIPTOR_BYTES]) {
    output[0..2].copy_from_slice(&contract.major.to_le_bytes());
    output[2..4].copy_from_slice(&contract.minor.to_le_bytes());
    output[4..6].copy_from_slice(&contract.descriptor.to_le_bytes());
    output[6..8].fill(0);
}

fn decode(bytes: &[u8; DESCRIPTOR_BYTES]) -> Result<Contract, crate::Error> {
    if bytes[6..8] != [0, 0] {
        return Err(crate::Error::Failed);
    }
    Ok(Contract::new(
        u16::from_le_bytes([bytes[0], bytes[1]]),
        u16::from_le_bytes([bytes[2], bytes[3]]),
        u16::from_le_bytes([bytes[4], bytes[5]]),
    ))
}

/// Negotiate the native contract through the kernel ABI.
pub fn negotiate_kernel(requested: Contract) -> Result<Contract, crate::Error> {
    let mut request = [0; DESCRIPTOR_BYTES];
    let mut response = [0; DESCRIPTOR_BYTES];
    encode(requested, &mut request);
    let length = crate::check(unsafe {
        crate::syscall::invoke(
            crate::syscall::ABI_NEGOTIATE,
            request.as_ptr() as u64,
            response.as_mut_ptr() as u64,
            response.len() as u64,
        )
    })?;
    if length != DESCRIPTOR_BYTES as u64 {
        return Err(crate::Error::Failed);
    }
    let negotiated = decode(&response)?;
    negotiate(negotiated).map_err(|_| crate::Error::Failed)
}

#[cfg(test)]
mod tests {
    use super::{
        ABI_MAJOR, ABI_MINOR, Contract, DESCRIPTOR_BYTES, DESCRIPTOR_VERSION, Error, decode,
        encode, negotiate,
    };

    #[test]
    fn current_contract_negotiates() {
        assert_eq!(negotiate(Contract::CURRENT), Ok(Contract::CURRENT));
    }

    #[test]
    fn older_minor_is_accepted_without_upgrade_claim() {
        let result = negotiate(Contract::new(
            ABI_MAJOR,
            ABI_MINOR.saturating_sub(1),
            DESCRIPTOR_VERSION,
        ));
        assert_eq!(
            result,
            Ok(Contract::new(
                ABI_MAJOR,
                ABI_MINOR.saturating_sub(1),
                DESCRIPTOR_VERSION
            ))
        );
    }

    #[test]
    fn unsupported_versions_fail_closed() {
        assert_eq!(
            negotiate(Contract::new(ABI_MAJOR + 1, 0, DESCRIPTOR_VERSION)),
            Err(Error::UnsupportedMajor {
                requested: ABI_MAJOR + 1,
                supported: ABI_MAJOR
            })
        );
        assert_eq!(
            negotiate(Contract::new(ABI_MAJOR, ABI_MINOR + 1, DESCRIPTOR_VERSION)),
            Err(Error::UnsupportedMinor {
                requested: ABI_MINOR + 1,
                supported: ABI_MINOR
            })
        );
        assert_eq!(
            negotiate(Contract::new(ABI_MAJOR, 0, DESCRIPTOR_VERSION + 1)),
            Err(Error::UnsupportedDescriptor {
                requested: DESCRIPTOR_VERSION + 1,
                supported: DESCRIPTOR_VERSION
            })
        );
    }

    #[test]
    fn descriptor_round_trip_and_reserved_bytes_are_bounded() {
        let contract = Contract::CURRENT;
        let mut bytes = [0; DESCRIPTOR_BYTES];
        encode(contract, &mut bytes);
        assert_eq!(decode(&bytes), Ok(contract));
        bytes[6] = 1;
        assert_eq!(decode(&bytes), Err(crate::Error::Failed));
    }
}
