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

#[cfg(test)]
mod tests {
    use super::{negotiate, Contract, Error, ABI_MAJOR, ABI_MINOR, DESCRIPTOR_VERSION};

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
}
