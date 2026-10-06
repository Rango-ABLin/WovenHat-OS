//! WovenHat Stage 9 IPv4 network stack.
//!
//! Stage 9 keeps the shell-first recovery path while exposing smoltcp sockets
//! to Ring-3 processes through a small kernel ABI.  The implementation uses
//! owned buffers so sockets can be created and destroyed dynamically without
//! static-lifetime bookkeeping in user processes.

use crate::irq_lock::IrqMutex as Mutex;
use alloc::{vec, vec::Vec};
use smoltcp::{
    iface::{Config, Interface, SocketHandle, SocketSet},
    phy::{ChecksumCapabilities, Device, DeviceCapabilities, Medium, RxToken, TxToken},
    socket::{dhcpv4, dns, icmp, tcp, udp},
    time::Instant,
    wire::{EthernetAddress, IpAddress, IpCidr, IpEndpoint, Ipv4Address},
};
use spin::Once;

#[cfg(feature = "stage14-2-test")]
use smoltcp::wire::Ipv6Address as SmolIpv6Address;

use crate::{
    timer,
    virtio_net::{self, MAX_FRAME},
};

#[cfg(feature = "stage13-9-test")]
use crate::{wifi_session::WifiSession, wifi_smol};

pub const DEFAULT_IPV4: Ipv4Address = Ipv4Address::new(10, 0, 2, 15);
pub const DEFAULT_GATEWAY: Ipv4Address = Ipv4Address::new(10, 0, 2, 2);
pub const DEFAULT_DNS: Ipv4Address = Ipv4Address::new(10, 0, 2, 3);
pub const DEFAULT_PREFIX: u8 = 24;
pub const MAX_USER_SOCKETS: usize = 16;
pub const SOCKET_BUFFER_BYTES: usize = 4096;
pub const UDP_META_SLOTS: usize = 8;
// Closed descriptors retain their TCP transport for a bounded graceful drain.
// This is a resource-retirement bound, not an acceptance-test timeout.
const TCP_CLOSE_GRACE_TICKS: u64 = timer::FREQUENCY_HZ as u64 * 30;

pub fn default_cidr() -> IpCidr {
    IpCidr::new(IpAddress::Ipv4(DEFAULT_IPV4), DEFAULT_PREFIX)
}

#[cfg(feature = "stage14-2-test")]
fn stage14_2_link_local_from_mac(mac: [u8; 6]) -> Ipv6Address {
    let mut bytes = [0u8; 16];
    bytes[0] = 0xfe;
    bytes[1] = 0x80;
    bytes[8] = mac[0] ^ 0x02;
    bytes[9] = mac[1];
    bytes[10] = mac[2];
    bytes[11] = 0xff;
    bytes[12] = 0xfe;
    bytes[13] = mac[3];
    bytes[14] = mac[4];
    bytes[15] = mac[5];
    Ipv6Address(bytes)
}

#[cfg(feature = "stage14-2-test")]
pub fn stage14_2_runtime_ipv6_self_test() -> bool {
    let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
    stage14_2_link_local_from_mac(mac)
        == Ipv6Address([
            0xfe, 0x80, 0, 0, 0, 0, 0, 0, 0x50, 0x54, 0x00, 0xff, 0xfe, 0x12, 0x34, 0x56,
        ])
}


#[cfg(feature = "stage14-2-test")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ipv6Prefix {
    pub address: [u8; 16],
    pub prefix_len: u8,
}

#[cfg(feature = "stage14-2-test")]
impl Ipv6Prefix {
    pub const fn new(address: [u8; 16], prefix_len: u8) -> Option<Self> {
        if prefix_len > 128 {
            return None;
        }
        Some(Self { address, prefix_len })
    }

    pub const fn is_unspecified(&self) -> bool {
        let mut index = 0;
        while index < self.address.len() {
            if self.address[index] != 0 {
                return false;
            }
            index += 1;
        }
        true
    }

    pub const fn is_link_local(&self) -> bool {
        self.address[0] == 0xfe && (self.address[1] & 0xc0) == 0x80
    }

    pub const fn is_multicast(&self) -> bool {
        self.address[0] == 0xff
    }

    pub const fn contains(&self, address: [u8; 16]) -> bool {
        let full_bytes = (self.prefix_len / 8) as usize;
        let remaining_bits = self.prefix_len % 8;
        let mut index = 0;
        while index < full_bytes {
            if self.address[index] != address[index] {
                return false;
            }
            index += 1;
        }
        if remaining_bits == 0 {
            return true;
        }
        let mask = 0xff << (8 - remaining_bits);
        (self.address[full_bytes] & mask) == (address[full_bytes] & mask)
    }
}

#[cfg(feature = "stage14-2-test")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ipv6Address(pub [u8; 16]);

#[cfg(feature = "stage14-2-test")]
impl Ipv6Address {
    pub const fn is_unspecified(self) -> bool {
        let mut index = 0;
        while index < self.0.len() {
            if self.0[index] != 0 {
                return false;
            }
            index += 1;
        }
        true
    }

    pub const fn is_link_local(self) -> bool {
        self.0[0] == 0xfe && (self.0[1] & 0xc0) == 0x80
    }

    pub const fn is_multicast(self) -> bool {
        self.0[0] == 0xff
    }

    /// RFC 4291 solicited-node multicast address for Neighbor Discovery.
    pub const fn solicited_node_multicast(self) -> Self {
        Self([
            0xff, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0xff, self.0[13], self.0[14],
            self.0[15],
        ])
    }
}

#[cfg(feature = "stage14-2-test")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeighborDiscoveryKind {
    RouterSolicitation,
    RouterAdvertisement,
    NeighborSolicitation,
    NeighborAdvertisement,
}

#[cfg(feature = "stage14-2-test")]
#[derive(Debug, PartialEq, Eq)]
pub struct NeighborDiscoveryMessage<'a> {
    pub kind: NeighborDiscoveryKind,
    pub target: Option<Ipv6Address>,
    pub option_bytes: &'a [u8],
}

#[cfg(feature = "stage14-2-test")]
fn valid_neighbor_options(mut bytes: &[u8]) -> bool {
    while !bytes.is_empty() {
        if bytes.len() < 2 {
            return false;
        }
        let length_units = bytes[1] as usize;
        if length_units == 0 {
            return false;
        }
        let option_len = length_units * 8;
        if option_len > bytes.len() {
            return false;
        }
        bytes = &bytes[option_len..];
    }
    true
}

/// Parse the bounded ICMPv6 Neighbor Discovery message envelope.
///
/// The caller owns the packet and remains responsible for validating the
/// IPv6 pseudo-header checksum before acting on the returned message. This
/// parser only accepts types 133–136, code zero, fixed-body lengths, and
/// complete non-zero-length options.
#[cfg(feature = "stage14-2-test")]
pub fn parse_icmpv6_neighbor_discovery(
    packet: &[u8],
) -> Option<NeighborDiscoveryMessage<'_>> {
    if packet.len() > 1024 || packet.len() < 2 {
        return None;
    }
    let kind = match (packet[0], packet[1]) {
        (133, 0) if packet.len() >= 8 => NeighborDiscoveryKind::RouterSolicitation,
        (134, 0) if packet.len() >= 16 => NeighborDiscoveryKind::RouterAdvertisement,
        (135, 0) if packet.len() >= 24 => NeighborDiscoveryKind::NeighborSolicitation,
        (136, 0) if packet.len() >= 24 => NeighborDiscoveryKind::NeighborAdvertisement,
        _ => return None,
    };

    let fixed_len = match kind {
        NeighborDiscoveryKind::RouterSolicitation => 8,
        NeighborDiscoveryKind::RouterAdvertisement => 16,
        NeighborDiscoveryKind::NeighborSolicitation
        | NeighborDiscoveryKind::NeighborAdvertisement => 24,
    };
    let target = if fixed_len == 24 {
        let mut bytes = [0; 16];
        bytes.copy_from_slice(&packet[8..24]);
        let address = Ipv6Address(bytes);
        if address.is_multicast() {
            return None;
        }
        Some(address)
    } else {
        None
    };
    let option_bytes = &packet[fixed_len..];
    valid_neighbor_options(option_bytes).then_some(NeighborDiscoveryMessage {
        kind,
        target,
        option_bytes,
    })
}

#[cfg(feature = "stage14-2-test")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RouterAdvertisementState {
    pub router: Option<Ipv6Address>,
    pub router_expires_at: u64,
    pub prefix: Option<Ipv6Prefix>,
    pub prefix_valid_until: u64,
    pub prefix_preferred_until: u64,
}

#[cfg(feature = "stage14-2-test")]
impl RouterAdvertisementState {
    pub const fn new() -> Self {
        Self {
            router: None,
            router_expires_at: 0,
            prefix: None,
            prefix_valid_until: 0,
            prefix_preferred_until: 0,
        }
    }

    pub fn expire(&mut self, now: u64) {
        if self.router.is_some() && now >= self.router_expires_at {
            self.router = None;
            self.router_expires_at = 0;
        }
        if self.prefix.is_some() && now >= self.prefix_valid_until {
            self.prefix = None;
            self.prefix_valid_until = 0;
            self.prefix_preferred_until = 0;
        } else if self.prefix.is_some() && now >= self.prefix_preferred_until {
            self.prefix_preferred_until = now;
        }
    }

    pub fn apply(
        &mut self,
        router: Ipv6Address,
        packet: &[u8],
        now: u64,
    ) -> bool {
        let Some(message) = parse_icmpv6_neighbor_discovery(packet) else {
            return false;
        };
        if message.kind != NeighborDiscoveryKind::RouterAdvertisement || !router.is_link_local() {
            return false;
        }

        let router_lifetime = u16::from_be_bytes([packet[6], packet[7]]) as u64;
        if router_lifetime == 0 {
            if self.router == Some(router) {
                self.router = None;
                self.router_expires_at = 0;
            }
        } else {
            self.router = Some(router);
            self.router_expires_at = now.saturating_add(router_lifetime);
        }

        let mut options = message.option_bytes;
        while !options.is_empty() {
            let option_len = options[1] as usize * 8;
            if options[0] == 3 && option_len == 32 {
                let prefix_len = options[2];
                let valid = u32::from_be_bytes([options[4], options[5], options[6], options[7]]) as u64;
                let preferred =
                    u32::from_be_bytes([options[8], options[9], options[10], options[11]]) as u64;
                if preferred > valid {
                    return false;
                }
                let mut bytes = [0u8; 16];
                bytes.copy_from_slice(&options[16..32]);
                let Some(prefix) = Ipv6Prefix::new(bytes, prefix_len) else {
                    return false;
                };
                if valid == 0 {
                    if self.prefix == Some(prefix) {
                        self.prefix = None;
                        self.prefix_valid_until = 0;
                        self.prefix_preferred_until = 0;
                    }
                } else {
                    self.prefix = Some(prefix);
                    self.prefix_valid_until = now.saturating_add(valid);
                    self.prefix_preferred_until = now.saturating_add(preferred);
                }
            }
            options = &options[option_len..];
        }
        true
    }
}

#[cfg(feature = "stage14-2-test")]
pub fn stage14_2_router_advertisement_state_self_test() -> bool {
    let router = Ipv6Address([0xfe, 0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
    let mut packet = [0u8; 48];
    packet[0] = 134;
    packet[6..8].copy_from_slice(&30u16.to_be_bytes());
    packet[16] = 3;
    packet[17] = 4;
    packet[18] = 64;
    packet[19] = 0xc0;
    packet[20..24].copy_from_slice(&120u32.to_be_bytes());
    packet[24..28].copy_from_slice(&60u32.to_be_bytes());
    packet[32..48].copy_from_slice(&[
        0x20, 0x01, 0x0d, 0xb8, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ]);

    let mut state = RouterAdvertisementState::new();
    if !state.apply(router, &packet, 100)
        || state.router != Some(router)
        || state.router_expires_at != 130
        || state.prefix.is_none()
        || state.prefix_valid_until != 220
        || state.prefix_preferred_until != 160
    {
        return false;
    }
    state.expire(160);
    if state.prefix.is_none() || state.prefix_preferred_until != 160 {
        return false;
    }
    state.expire(130);
    if state.router.is_some() || state.prefix.is_none() {
        return false;
    }
    state.expire(220);
    if state.prefix.is_some() {
        return false;
    }

    let mut invalid = packet;
    invalid[20..24].copy_from_slice(&10u32.to_be_bytes());
    invalid[24..28].copy_from_slice(&11u32.to_be_bytes());
    if state.apply(router, &invalid, 300) {
        return false;
    }

    let global_router =
        Ipv6Address([0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
    if state.apply(global_router, &packet, 300) {
        return false;
    }

    let mut withdraw = packet;
    withdraw[6..8].copy_from_slice(&0u16.to_be_bytes());
    withdraw[20..24].copy_from_slice(&0u32.to_be_bytes());
    withdraw[24..28].copy_from_slice(&0u32.to_be_bytes());
    state.apply(router, &packet, 400)
        && state.apply(router, &withdraw, 401)
        && state.router.is_none()
        && state.prefix.is_none()
}

#[cfg(feature = "stage14-2-test")]
pub fn stage14_2_ipv6_foundation_self_test() -> bool {
    let link_local = Ipv6Prefix::new(
        [0xfe, 0x80, 0, 0, 0, 0, 0, 0, 0x02, 0, 0xff, 0xfe, 0, 0, 0, 1],
        64,
    );
    let unspecified = Ipv6Prefix::new([0; 16], 128);
    let multicast = Ipv6Prefix::new(
        [0xff, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
        128,
    );

    matches!(link_local, Some(prefix) if prefix.is_link_local() && !prefix.is_multicast())
        && matches!(unspecified, Some(prefix) if prefix.is_unspecified() && !prefix.is_link_local())
        && matches!(multicast, Some(prefix) if prefix.is_multicast() && !prefix.is_unspecified())
        && Ipv6Prefix::new([0; 16], 129).is_none()
}

#[cfg(feature = "stage14-2-test")]
pub fn stage14_2_ipv6_neighbor_foundation_self_test() -> bool {
    let address = Ipv6Address([
        0xfe, 0x80, 0, 0, 0, 0, 0, 0, 0x02, 0, 0, 0xff, 0xfe, 0, 0, 1,
    ]);
    let link_local = Ipv6Prefix::new(
        [0xfe, 0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        64,
    );
    let solicited = address.solicited_node_multicast();
    address.is_link_local()
        && !address.is_multicast()
        && link_local.is_some_and(|prefix| prefix.contains(address.0))
        && solicited
            == Ipv6Address([0xff, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0xff, 0, 0, 1])
        && solicited.is_multicast()
}

#[cfg(feature = "stage14-2-test")]
pub fn stage14_2_icmpv6_neighbor_parser_self_test() -> bool {
    let mut solicitation = [0u8; 32];
    solicitation[0] = 135;
    solicitation[1] = 0;
    solicitation[8..24].copy_from_slice(&[
        0xfe, 0x80, 0, 0, 0, 0, 0, 0, 0x02, 0, 0, 0xff, 0xfe, 0, 0, 1,
    ]);
    solicitation[24] = 1;
    solicitation[25] = 1;

    let mut truncated_option = solicitation;
    truncated_option[25] = 2;
    let mut zero_length_option = solicitation;
    zero_length_option[25] = 0;
    let mut multicast_target = solicitation;
    multicast_target[8] = 0xff;
    let router_advertisement = [134, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

    matches!(
        parse_icmpv6_neighbor_discovery(&solicitation),
        Some(NeighborDiscoveryMessage {
            kind: NeighborDiscoveryKind::NeighborSolicitation,
            target: Some(_),
            option_bytes,
        }) if option_bytes.len() == 8
    ) && parse_icmpv6_neighbor_discovery(&truncated_option).is_none()
        && parse_icmpv6_neighbor_discovery(&zero_length_option).is_none()
        && parse_icmpv6_neighbor_discovery(&multicast_target).is_none()
        && matches!(
            parse_icmpv6_neighbor_discovery(&router_advertisement),
            Some(NeighborDiscoveryMessage {
                kind: NeighborDiscoveryKind::RouterAdvertisement,
                target: None,
                option_bytes,
            }) if option_bytes.is_empty()
        )
        && parse_icmpv6_neighbor_discovery(&[135, 1, 0, 0]).is_none()
}

#[cfg(feature = "stage14-2-test")]
fn checksum_add(mut sum: u32, value: u16) -> u32 {
    sum += value as u32;
    (sum & 0xffff) + (sum >> 16)
}

#[cfg(feature = "stage14-2-test")]
fn checksum_bytes(mut sum: u32, bytes: &[u8]) -> u32 {
    let mut index = 0;
    while index + 1 < bytes.len() {
        sum = checksum_add(sum, u16::from_be_bytes([bytes[index], bytes[index + 1]]));
        index += 2;
    }
    if index < bytes.len() {
        sum = checksum_add(sum, u16::from_be_bytes([bytes[index], 0]));
    }
    sum
}

/// Calculate the ICMPv6 checksum over the IPv6 pseudo-header and packet.
/// The packet checksum field (bytes 2..4) is treated as zero.
#[cfg(feature = "stage14-2-test")]
pub fn icmpv6_checksum(source: Ipv6Address, destination: Ipv6Address, packet: &[u8]) -> u16 {
    if packet.len() < 4 || packet.len() > 1024 {
        return 0;
    }
    let mut sum = checksum_bytes(0, &source.0);
    sum = checksum_bytes(sum, &destination.0);
    sum = checksum_add(sum, (packet.len() >> 16) as u16);
    sum = checksum_add(sum, packet.len() as u16);
    sum = checksum_add(sum, 58);
    sum = checksum_add(sum, 0);
    sum = checksum_add(sum, u16::from_be_bytes([packet[0], packet[1]]));
    sum = checksum_add(sum, 0);
    sum = checksum_bytes(sum, &packet[4..]);
    let folded = (sum & 0xffff) + (sum >> 16);
    !(folded as u16)
}

#[cfg(feature = "stage14-2-test")]
pub fn verify_icmpv6_checksum(
    source: Ipv6Address,
    destination: Ipv6Address,
    packet: &[u8],
) -> bool {
    packet.len() >= 4
        && packet.len() <= 1024
        && u16::from_be_bytes([packet[2], packet[3]]) == icmpv6_checksum(source, destination, packet)
}

#[cfg(feature = "stage14-2-test")]
pub fn parse_checked_icmpv6_neighbor_discovery(
    source: Ipv6Address,
    destination: Ipv6Address,
    packet: &[u8],
) -> Option<NeighborDiscoveryMessage<'_>> {
    verify_icmpv6_checksum(source, destination, packet)
        .then(|| parse_icmpv6_neighbor_discovery(packet))
        .flatten()
}

#[cfg(feature = "stage14-2-test")]
fn parse_validated_icmpv6_neighbor_discovery(
    source: Ipv6Address,
    destination: Ipv6Address,
    hop_limit: u8,
    packet: &[u8],
) -> Option<NeighborDiscoveryMessage<'_>> {
    if hop_limit != 255 {
        return None;
    }
    let message = parse_checked_icmpv6_neighbor_discovery(source, destination, packet)?;
    match message.kind {
        NeighborDiscoveryKind::RouterAdvertisement
            if !source.is_link_local() || destination.is_unspecified() =>
        {
            None
        }
        NeighborDiscoveryKind::NeighborAdvertisement
            if source.is_unspecified() || destination.is_unspecified() =>
        {
            None
        }
        NeighborDiscoveryKind::NeighborSolicitation
            if source.is_unspecified() && destination != message.target?.solicited_node_multicast() =>
        {
            None
        }
        _ => Some(message),
    }
}

#[cfg(feature = "stage14-2-test")]
pub fn stage14_2_ndp_ingress_hardening_self_test() -> bool {
    let peer = Ipv6Address([
        0xfe, 0x80, 0, 0, 0, 0, 0, 0, 0x02, 0, 0, 0xff, 0xfe, 0, 0, 2,
    ]);
    let destination = peer.solicited_node_multicast();
    let mut advertisement = [0u8; 24];
    advertisement[0] = 136;
    advertisement[8..24].copy_from_slice(&peer.0);
    finish_icmpv6_checksum(peer, destination, &mut advertisement);
    if parse_validated_icmpv6_neighbor_discovery(peer, destination, 255, &advertisement).is_none()
        || parse_validated_icmpv6_neighbor_discovery(peer, destination, 64, &advertisement).is_some()
    {
        return false;
    }

    let unspecified = Ipv6Address([0; 16]);
    let mut solicitation = [0u8; 24];
    solicitation[0] = 135;
    solicitation[8..24].copy_from_slice(&peer.0);
    finish_icmpv6_checksum(unspecified, destination, &mut solicitation);
    if parse_validated_icmpv6_neighbor_discovery(
        unspecified,
        destination,
        255,
        &solicitation,
    )
    .is_none()
    {
        return false;
    }
    let all_nodes = Ipv6Address([0xff, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
    finish_icmpv6_checksum(unspecified, all_nodes, &mut solicitation);
    parse_validated_icmpv6_neighbor_discovery(unspecified, all_nodes, 255, &solicitation).is_none()
}

#[cfg(feature = "stage14-2-test")]
pub fn stage14_2_icmpv6_checksum_self_test() -> bool {
    let source = Ipv6Address([
        0xfe, 0x80, 0, 0, 0, 0, 0, 0, 0x02, 0, 0, 0xff, 0xfe, 0, 0, 1,
    ]);
    let destination = source.solicited_node_multicast();
    let mut packet = [0u8; 8];
    packet[0] = 133;
    let checksum = icmpv6_checksum(source, destination, &packet);
    packet[2..4].copy_from_slice(&checksum.to_be_bytes());
    let valid = verify_icmpv6_checksum(source, destination, &packet)
        && matches!(
            parse_checked_icmpv6_neighbor_discovery(source, destination, &packet),
            Some(NeighborDiscoveryMessage {
                kind: NeighborDiscoveryKind::RouterSolicitation,
                target: None,
                option_bytes,
            }) if option_bytes.is_empty()
        );
    packet[7] = 1;
    valid
        && !verify_icmpv6_checksum(source, destination, &packet)
        && parse_checked_icmpv6_neighbor_discovery(source, destination, &packet).is_none()
}

#[cfg(feature = "stage14-2-test")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ipv6NeighborState {
    Incomplete,
    Reachable,
    Stale,
}

#[cfg(feature = "stage14-2-test")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ipv6NeighborEntry {
    pub address: Ipv6Address,
    pub state: Ipv6NeighborState,
    pub reachable_until: u64,
}

#[cfg(feature = "stage14-2-test")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DuplicateAddressDetection {
    pub tentative: Ipv6Address,
    pub conflict: bool,
    active: bool,
}

#[cfg(feature = "stage14-2-test")]
impl DuplicateAddressDetection {
    pub const fn new(tentative: Ipv6Address) -> Self {
        Self {
            tentative,
            conflict: false,
            active: false,
        }
    }

    pub fn arm(&mut self) {
        self.active = true;
        self.conflict = false;
    }

    pub fn observe_checked(
        &mut self,
        source: Ipv6Address,
        destination: Ipv6Address,
        packet: &[u8],
    ) -> bool {
        let Some(message) =
            parse_checked_icmpv6_neighbor_discovery(source, destination, packet)
        else {
            return false;
        };
        if self.active && matches!(
            message.kind,
            NeighborDiscoveryKind::NeighborSolicitation
                | NeighborDiscoveryKind::NeighborAdvertisement
        ) && message.target == Some(self.tentative)
        {
            self.conflict = true;
            self.active = false;
        }
        true
    }
}

#[cfg(feature = "stage14-2-test")]
impl Ipv6NeighborEntry {
    pub const fn new(address: Ipv6Address) -> Self {
        Self {
            address,
            state: Ipv6NeighborState::Incomplete,
            reachable_until: 0,
        }
    }

    pub fn observe_checked(
        &mut self,
        source: Ipv6Address,
        destination: Ipv6Address,
        packet: &[u8],
        now: u64,
        reachable_lifetime: u64,
    ) -> bool {
        let Some(message) =
            parse_checked_icmpv6_neighbor_discovery(source, destination, packet)
        else {
            return false;
        };
        match message.kind {
            NeighborDiscoveryKind::NeighborAdvertisement
                if message.target == Some(self.address) =>
            {
                self.state = Ipv6NeighborState::Reachable;
                self.reachable_until = now.saturating_add(reachable_lifetime);
            }
            NeighborDiscoveryKind::NeighborSolicitation if source == self.address => {
                self.state = Ipv6NeighborState::Stale;
                self.reachable_until = 0;
            }
            _ => {}
        }
        true
    }

    pub fn expire(&mut self, now: u64) {
        if self.state == Ipv6NeighborState::Reachable && now >= self.reachable_until {
            self.state = Ipv6NeighborState::Stale;
            self.reachable_until = 0;
        }
    }
}

#[cfg(feature = "stage14-2-test")]
fn finish_icmpv6_checksum(
    source: Ipv6Address,
    destination: Ipv6Address,
    packet: &mut [u8],
) {
    packet[2] = 0;
    packet[3] = 0;
    let checksum = icmpv6_checksum(source, destination, packet);
    packet[2..4].copy_from_slice(&checksum.to_be_bytes());
}

#[cfg(feature = "stage14-2-test")]
pub fn stage14_2_neighbor_state_dad_self_test() -> bool {
    let local = Ipv6Address([
        0xfe, 0x80, 0, 0, 0, 0, 0, 0, 0x02, 0, 0, 0xff, 0xfe, 0, 0, 1,
    ]);
    let peer = Ipv6Address([
        0xfe, 0x80, 0, 0, 0, 0, 0, 0, 0x02, 0, 0, 0xff, 0xfe, 0, 0, 2,
    ]);
    let all_nodes = Ipv6Address([0xff, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

    let mut advertisement = [0u8; 24];
    advertisement[0] = 136;
    advertisement[8..24].copy_from_slice(&peer.0);
    finish_icmpv6_checksum(peer, all_nodes, &mut advertisement);

    let mut entry = Ipv6NeighborEntry::new(peer);
    if !entry.observe_checked(peer, all_nodes, &advertisement, 100, 30)
        || entry.state != Ipv6NeighborState::Reachable
        || entry.reachable_until != 130
    {
        return false;
    }
    entry.expire(130);
    if entry.state != Ipv6NeighborState::Stale {
        return false;
    }

    let unspecified = Ipv6Address([0; 16]);
    let solicited = local.solicited_node_multicast();
    let mut solicitation = [0u8; 24];
    solicitation[0] = 135;
    solicitation[8..24].copy_from_slice(&local.0);
    finish_icmpv6_checksum(unspecified, solicited, &mut solicitation);

    let mut dad = DuplicateAddressDetection::new(local);
    dad.arm();
    if !dad.observe_checked(unspecified, solicited, &solicitation) || !dad.conflict {
        return false;
    }

    let mut tampered = solicitation;
    tampered[7] ^= 1;
    let mut clean_dad = DuplicateAddressDetection::new(local);
    !clean_dad.observe_checked(unspecified, solicited, &tampered) && !clean_dad.conflict
}

#[cfg(feature = "stage14-2-test")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlaacAddressState {
    Tentative,
    Preferred,
    Deprecated,
    Duplicate,
    Expired,
}

#[cfg(feature = "stage14-2-test")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlaacAddress {
    pub address: Ipv6Address,
    pub state: SlaacAddressState,
    pub preferred_until: u64,
    pub valid_until: u64,
}

#[cfg(feature = "stage14-2-test")]
impl SlaacAddress {
    pub fn from_prefix(
        prefix: Ipv6Prefix,
        interface_id: [u8; 8],
        preferred_until: u64,
        valid_until: u64,
    ) -> Option<Self> {
        if prefix.prefix_len != 64
            || prefix.is_multicast()
            || prefix.is_link_local()
            || preferred_until > valid_until
        {
            return None;
        }
        let mut bytes = prefix.address;
        bytes[8..16].copy_from_slice(&interface_id);
        Some(Self {
            address: Ipv6Address(bytes),
            state: SlaacAddressState::Tentative,
            preferred_until,
            valid_until,
        })
    }

    pub fn complete_dad(&mut self, dad: DuplicateAddressDetection, now: u64) -> bool {
        if dad.tentative != self.address || self.state != SlaacAddressState::Tentative {
            return false;
        }
        self.state = if dad.conflict {
            SlaacAddressState::Duplicate
        } else if now >= self.valid_until {
            SlaacAddressState::Expired
        } else if now >= self.preferred_until {
            SlaacAddressState::Deprecated
        } else {
            SlaacAddressState::Preferred
        };
        true
    }

    pub fn expire(&mut self, now: u64) {
        if matches!(
            self.state,
            SlaacAddressState::Duplicate | SlaacAddressState::Expired
        ) {
            return;
        }
        if now >= self.valid_until {
            self.state = SlaacAddressState::Expired;
        } else if now >= self.preferred_until
            && self.state != SlaacAddressState::Tentative
        {
            self.state = SlaacAddressState::Deprecated;
        }
    }
}

#[cfg(feature = "stage14-2-test")]
pub fn stage14_2_slaac_self_test() -> bool {
    let prefix = Ipv6Prefix::new(
        [0x20, 0x01, 0x0d, 0xb8, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        64,
    )
    .expect("valid SLAAC prefix");
    let interface_id = [0x02, 0, 0xff, 0xfe, 0, 0, 0, 1];
    let Some(mut address) = SlaacAddress::from_prefix(prefix, interface_id, 160, 220) else {
        return false;
    };
    if address.address.0
        != [0x20, 0x01, 0x0d, 0xb8, 0, 1, 0, 0, 0x02, 0, 0xff, 0xfe, 0, 0, 0, 1]
        || address.state != SlaacAddressState::Tentative
    {
        return false;
    }

    let clean_dad = DuplicateAddressDetection::new(address.address);
    if !address.complete_dad(clean_dad, 120)
        || address.state != SlaacAddressState::Preferred
    {
        return false;
    }
    address.expire(160);
    if address.state != SlaacAddressState::Deprecated {
        return false;
    }
    address.expire(220);
    if address.state != SlaacAddressState::Expired {
        return false;
    }

    let Some(mut duplicate) = SlaacAddress::from_prefix(prefix, interface_id, 360, 420) else {
        return false;
    };
    let mut conflict = DuplicateAddressDetection::new(duplicate.address);
    conflict.conflict = true;
    if !duplicate.complete_dad(conflict, 320)
        || duplicate.state != SlaacAddressState::Duplicate
    {
        return false;
    }

    let non_64 = Ipv6Prefix::new(prefix.address, 56).expect("valid IPv6 prefix");
    SlaacAddress::from_prefix(non_64, interface_id, 10, 20).is_none()
        && SlaacAddress::from_prefix(prefix, interface_id, 21, 20).is_none()
}

#[cfg(feature = "stage14-2-test")]
pub const DHCPV6_CLIENT_PORT: u16 = 546;
#[cfg(feature = "stage14-2-test")]
pub const DHCPV6_SERVER_PORT: u16 = 547;
#[cfg(feature = "stage14-2-test")]
const DHCPV6_MAX_MESSAGE: usize = 1232;

#[cfg(feature = "stage14-2-test")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Dhcpv6MessageType {
    Solicit = 1,
    Advertise = 2,
    Request = 3,
    Confirm = 4,
    Renew = 5,
    Rebind = 6,
    Reply = 7,
    Release = 8,
    Decline = 9,
    Reconfigure = 10,
    InformationRequest = 11,
}

#[cfg(feature = "stage14-2-test")]
impl Dhcpv6MessageType {
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Solicit),
            2 => Some(Self::Advertise),
            3 => Some(Self::Request),
            4 => Some(Self::Confirm),
            5 => Some(Self::Renew),
            6 => Some(Self::Rebind),
            7 => Some(Self::Reply),
            8 => Some(Self::Release),
            9 => Some(Self::Decline),
            10 => Some(Self::Reconfigure),
            11 => Some(Self::InformationRequest),
            _ => None,
        }
    }
}

#[cfg(feature = "stage14-2-test")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dhcpv6Message<'a> {
    pub message_type: Dhcpv6MessageType,
    pub transaction_id: u32,
    pub options: &'a [u8],
}

#[cfg(feature = "stage14-2-test")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dhcpv6Option<'a> {
    pub code: u16,
    pub value: &'a [u8],
}

#[cfg(feature = "stage14-2-test")]
fn valid_dhcpv6_options(mut bytes: &[u8]) -> bool {
    while !bytes.is_empty() {
        if bytes.len() < 4 {
            return false;
        }
        let length = u16::from_be_bytes([bytes[2], bytes[3]]) as usize;
        let Some(total) = 4usize.checked_add(length) else {
            return false;
        };
        if total > bytes.len() {
            return false;
        }
        bytes = &bytes[total..];
    }
    true
}

#[cfg(feature = "stage14-2-test")]
pub fn parse_dhcpv6_message(packet: &[u8]) -> Option<Dhcpv6Message<'_>> {
    if packet.len() < 4 || packet.len() > DHCPV6_MAX_MESSAGE {
        return None;
    }
    let message_type = Dhcpv6MessageType::from_u8(packet[0])?;
    let transaction_id =
        (u32::from(packet[1]) << 16) | (u32::from(packet[2]) << 8) | u32::from(packet[3]);
    let options = &packet[4..];
    valid_dhcpv6_options(options).then_some(Dhcpv6Message {
        message_type,
        transaction_id,
        options,
    })
}

#[cfg(feature = "stage14-2-test")]
pub fn dhcpv6_find_option<'a>(message: &'a Dhcpv6Message<'a>, code: u16) -> Option<Dhcpv6Option<'a>> {
    let mut bytes = message.options;
    while !bytes.is_empty() {
        let option_code = u16::from_be_bytes([bytes[0], bytes[1]]);
        let length = u16::from_be_bytes([bytes[2], bytes[3]]) as usize;
        let total = 4 + length;
        if option_code == code {
            return Some(Dhcpv6Option {
                code: option_code,
                value: &bytes[4..total],
            });
        }
        bytes = &bytes[total..];
    }
    None
}

#[cfg(feature = "stage14-2-test")]
pub fn write_dhcpv6_message(
    output: &mut [u8],
    message_type: Dhcpv6MessageType,
    transaction_id: u32,
    options: &[u8],
) -> Option<usize> {
    if transaction_id > 0x00ff_ffff
        || !valid_dhcpv6_options(options)
        || options.len() + 4 > DHCPV6_MAX_MESSAGE
        || output.len() < options.len() + 4
    {
        return None;
    }
    output[0] = message_type as u8;
    output[1] = ((transaction_id >> 16) & 0xff) as u8;
    output[2] = ((transaction_id >> 8) & 0xff) as u8;
    output[3] = (transaction_id & 0xff) as u8;
    output[4..4 + options.len()].copy_from_slice(options);
    Some(4 + options.len())
}

#[cfg(feature = "stage14-2-test")]
pub fn stage14_2_dhcpv6_protocol_self_test() -> bool {
    let options = [
        0x00, 0x01, 0x00, 0x04, 0xde, 0xad, 0xbe, 0xef,
        0x00, 0x06, 0x00, 0x02, 0x00, 0x17,
    ];
    let mut packet = [0u8; 32];
    let Some(length) = write_dhcpv6_message(
        &mut packet,
        Dhcpv6MessageType::Solicit,
        0x00a1_b2c3,
        &options,
    ) else {
        return false;
    };
    let Some(message) = parse_dhcpv6_message(&packet[..length]) else {
        return false;
    };
    if message.message_type != Dhcpv6MessageType::Solicit
        || message.transaction_id != 0x00a1_b2c3
        || dhcpv6_find_option(&message, 1).map(|option| option.value) != Some(&[0xde, 0xad, 0xbe, 0xef][..])
        || dhcpv6_find_option(&message, 6).map(|option| option.value) != Some(&[0x00, 0x17][..])
        || DHCPV6_CLIENT_PORT != 546
        || DHCPV6_SERVER_PORT != 547
    {
        return false;
    }

    let truncated = [1, 0, 0, 1, 0, 1, 0, 4, 0xaa];
    let unsupported = [12, 0, 0, 1];
    parse_dhcpv6_message(&truncated).is_none()
        && parse_dhcpv6_message(&unsupported).is_none()
        && write_dhcpv6_message(
            &mut packet,
            Dhcpv6MessageType::Request,
            0x0100_0000,
            &[],
        )
        .is_none()
}

#[cfg(feature = "stage14-2-test")]
const DHCPV6_OPTION_CLIENT_ID: u16 = 1;
#[cfg(feature = "stage14-2-test")]
const DHCPV6_OPTION_SERVER_ID: u16 = 2;
#[cfg(feature = "stage14-2-test")]
const DHCPV6_MAX_DUID: usize = 32;

#[cfg(feature = "stage14-2-test")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dhcpv6ClientState {
    Init,
    Soliciting,
    Requesting,
    Bound,
}

#[cfg(feature = "stage14-2-test")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dhcpv6Duid {
    bytes: [u8; DHCPV6_MAX_DUID],
    len: u8,
}

#[cfg(feature = "stage14-2-test")]
impl Dhcpv6Duid {
    pub fn new(value: &[u8]) -> Option<Self> {
        if value.is_empty() || value.len() > DHCPV6_MAX_DUID {
            return None;
        }
        let mut bytes = [0u8; DHCPV6_MAX_DUID];
        bytes[..value.len()].copy_from_slice(value);
        Some(Self {
            bytes,
            len: value.len() as u8,
        })
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len as usize]
    }
}

#[cfg(feature = "stage14-2-test")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dhcpv6Client {
    pub state: Dhcpv6ClientState,
    pub transaction_id: u32,
    client_id: Dhcpv6Duid,
    server_id: Option<Dhcpv6Duid>,
    pub retry_count: u8,
    pub next_retry_at: u64,
}

#[cfg(feature = "stage14-2-test")]
impl Dhcpv6Client {
    pub fn new(transaction_id: u32, client_id: &[u8]) -> Option<Self> {
        if transaction_id > 0x00ff_ffff {
            return None;
        }
        Some(Self {
            state: Dhcpv6ClientState::Init,
            transaction_id,
            client_id: Dhcpv6Duid::new(client_id)?,
            server_id: None,
            retry_count: 0,
            next_retry_at: 0,
        })
    }

    pub fn start(&mut self, now: u64, retry_after: u64) -> bool {
        if self.state != Dhcpv6ClientState::Init {
            return false;
        }
        self.state = Dhcpv6ClientState::Soliciting;
        self.retry_count = 0;
        self.next_retry_at = now.saturating_add(retry_after);
        true
    }

    pub fn retry_due(&mut self, now: u64, retry_after: u64) -> bool {
        if !matches!(
            self.state,
            Dhcpv6ClientState::Soliciting | Dhcpv6ClientState::Requesting
        ) || now < self.next_retry_at
        {
            return false;
        }
        self.retry_count = self.retry_count.saturating_add(1);
        self.next_retry_at = now.saturating_add(retry_after);
        true
    }

    fn identifiers_match(&self, message: &Dhcpv6Message<'_>) -> bool {
        dhcpv6_find_option(message, DHCPV6_OPTION_CLIENT_ID)
            .is_some_and(|option| option.value == self.client_id.as_slice())
    }

    pub fn observe(&mut self, packet: &[u8], now: u64, retry_after: u64) -> bool {
        let Some(message) = parse_dhcpv6_message(packet) else {
            return false;
        };
        if message.transaction_id != self.transaction_id || !self.identifiers_match(&message) {
            return false;
        }

        match (self.state, message.message_type) {
            (Dhcpv6ClientState::Soliciting, Dhcpv6MessageType::Advertise) => {
                let Some(server) = dhcpv6_find_option(&message, DHCPV6_OPTION_SERVER_ID)
                    .and_then(|option| Dhcpv6Duid::new(option.value))
                else {
                    return false;
                };
                self.server_id = Some(server);
                self.state = Dhcpv6ClientState::Requesting;
                self.retry_count = 0;
                self.next_retry_at = now.saturating_add(retry_after);
                true
            }
            (Dhcpv6ClientState::Requesting, Dhcpv6MessageType::Reply) => {
                let Some(server) = dhcpv6_find_option(&message, DHCPV6_OPTION_SERVER_ID) else {
                    return false;
                };
                if self.server_id.is_none_or(|expected| expected.as_slice() != server.value) {
                    return false;
                }
                self.state = Dhcpv6ClientState::Bound;
                self.retry_count = 0;
                self.next_retry_at = 0;
                true
            }
            _ => false,
        }
    }
}

#[cfg(feature = "stage14-2-test")]
fn write_dhcpv6_option(output: &mut [u8], code: u16, value: &[u8]) -> Option<usize> {
    if value.len() > u16::MAX as usize || output.len() < 4 + value.len() {
        return None;
    }
    output[..2].copy_from_slice(&code.to_be_bytes());
    output[2..4].copy_from_slice(&(value.len() as u16).to_be_bytes());
    output[4..4 + value.len()].copy_from_slice(value);
    Some(4 + value.len())
}

#[cfg(feature = "stage14-2-test")]
pub fn stage14_2_dhcpv6_client_self_test() -> bool {
    let client_id = [0, 3, 0, 1, 0x02, 0, 0, 0, 0, 1];
    let server_id = [0, 3, 0, 1, 0x02, 0, 0, 0, 0, 2];
    let transaction_id = 0x0012_3456;
    let Some(mut client) = Dhcpv6Client::new(transaction_id, &client_id) else {
        return false;
    };
    if !client.start(100, 10)
        || client.state != Dhcpv6ClientState::Soliciting
        || client.retry_due(109, 10)
        || !client.retry_due(110, 20)
        || client.retry_count != 1
        || client.next_retry_at != 130
    {
        return false;
    }

    let mut options = [0u8; 64];
    let Some(client_len) = write_dhcpv6_option(&mut options, DHCPV6_OPTION_CLIENT_ID, &client_id) else {
        return false;
    };
    let Some(server_len) = write_dhcpv6_option(
        &mut options[client_len..],
        DHCPV6_OPTION_SERVER_ID,
        &server_id,
    ) else {
        return false;
    };
    let options_len = client_len + server_len;
    let mut packet = [0u8; 96];
    let Some(advertise_len) = write_dhcpv6_message(
        &mut packet,
        Dhcpv6MessageType::Advertise,
        transaction_id,
        &options[..options_len],
    ) else {
        return false;
    };
    if !client.observe(&packet[..advertise_len], 120, 15)
        || client.state != Dhcpv6ClientState::Requesting
        || client.retry_count != 0
        || client.next_retry_at != 135
    {
        return false;
    }

    let Some(reply_len) = write_dhcpv6_message(
        &mut packet,
        Dhcpv6MessageType::Reply,
        transaction_id,
        &options[..options_len],
    ) else {
        return false;
    };
    if !client.observe(&packet[..reply_len], 125, 15)
        || client.state != Dhcpv6ClientState::Bound
        || client.next_retry_at != 0
    {
        return false;
    }

    let Some(mut wrong_order) = Dhcpv6Client::new(transaction_id, &client_id) else {
        return false;
    };
    if !wrong_order.start(0, 10) || wrong_order.observe(&packet[..reply_len], 1, 10) {
        return false;
    }

    let mut wrong_server_options = options;
    wrong_server_options[client_len + 4 + server_id.len() - 1] ^= 1;
    let Some(wrong_reply_len) = write_dhcpv6_message(
        &mut packet,
        Dhcpv6MessageType::Reply,
        transaction_id,
        &wrong_server_options[..options_len],
    ) else {
        return false;
    };
    let Some(mut mismatch) = Dhcpv6Client::new(transaction_id, &client_id) else {
        return false;
    };
    mismatch.state = Dhcpv6ClientState::Requesting;
    mismatch.server_id = Dhcpv6Duid::new(&server_id);
    !mismatch.observe(&packet[..wrong_reply_len], 1, 10)
        && Dhcpv6Client::new(0x0100_0000, &client_id).is_none()
        && Dhcpv6Client::new(transaction_id, &[]).is_none()
}

#[cfg(feature = "stage14-2-test")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ipv6IngressEvent {
    Ignored,
    RouterAdvertisement,
    NeighborAdvertisement,
    NeighborSolicitation,
    DuplicateAddress,
}

#[cfg(feature = "stage14-2-test")]
pub struct Ipv6InterfaceState {
    pub local: Ipv6Address,
    pub router: RouterAdvertisementState,
    pub neighbor: Ipv6NeighborEntry,
    pub dad: DuplicateAddressDetection,
}

#[cfg(feature = "stage14-2-test")]
impl Ipv6InterfaceState {
    pub fn new(local: Ipv6Address) -> Self {
        Self {
            local,
            router: RouterAdvertisementState::new(),
            neighbor: Ipv6NeighborEntry::new(local),
            dad: DuplicateAddressDetection::new(local),
        }
    }

    pub fn receive_icmpv6(
        &mut self,
        source: Ipv6Address,
        destination: Ipv6Address,
        packet: &[u8],
        now: u64,
    ) -> Ipv6IngressEvent {
        self.receive_icmpv6_with_hop_limit(source, destination, 255, packet, now)
    }

    pub fn receive_icmpv6_with_hop_limit(
        &mut self,
        source: Ipv6Address,
        destination: Ipv6Address,
        hop_limit: u8,
        packet: &[u8],
        now: u64,
    ) -> Ipv6IngressEvent {
        let Some(message) =
            parse_validated_icmpv6_neighbor_discovery(source, destination, hop_limit, packet)
        else {
            return Ipv6IngressEvent::Ignored;
        };
        match message.kind {
            NeighborDiscoveryKind::RouterAdvertisement => {
                if self.router.apply(source, packet, now) {
                    Ipv6IngressEvent::RouterAdvertisement
                } else {
                    Ipv6IngressEvent::Ignored
                }
            }
            NeighborDiscoveryKind::NeighborAdvertisement => {
                if !self.neighbor.observe_checked(source, destination, packet, now, 30) {
                    return Ipv6IngressEvent::Ignored;
                }
                let _ = self.dad.observe_checked(source, destination, packet);
                if self.dad.conflict { Ipv6IngressEvent::DuplicateAddress } else {
                    Ipv6IngressEvent::NeighborAdvertisement
                }
            }
            NeighborDiscoveryKind::NeighborSolicitation => {
                if !self.dad.observe_checked(source, destination, packet) {
                    return Ipv6IngressEvent::Ignored;
                }
                if self.dad.conflict { Ipv6IngressEvent::DuplicateAddress } else {
                    Ipv6IngressEvent::NeighborSolicitation
                }
            }
            NeighborDiscoveryKind::RouterSolicitation => Ipv6IngressEvent::Ignored,
        }
    }
}

#[cfg(feature = "stage14-2-test")]
pub fn stage14_2_ipv6_interface_ingress_self_test() -> bool {
    let peer = Ipv6Address([
        0xfe, 0x80, 0, 0, 0, 0, 0, 0, 0x02, 0, 0, 0xff, 0xfe, 0, 0, 2,
    ]);
    let destination = peer.solicited_node_multicast();
    let mut advertisement = [0u8; 24];
    advertisement[0] = 136;
    advertisement[8..24].copy_from_slice(&peer.0);
    finish_icmpv6_checksum(peer, destination, &mut advertisement);
    let mut state = Ipv6InterfaceState::new(peer);
    if state.receive_icmpv6(peer, destination, &advertisement, 100)
        != Ipv6IngressEvent::NeighborAdvertisement
        || state.neighbor.state != Ipv6NeighborState::Reachable
    { return false; }
    let mut tampered = advertisement;
    tampered[7] ^= 1;
    if state.receive_icmpv6(peer, destination, &tampered, 101) != Ipv6IngressEvent::Ignored {
        return false;
    }
    let unspecified = Ipv6Address([0; 16]);
    let mut solicitation = [0u8; 24];
    solicitation[0] = 135;
    solicitation[8..24].copy_from_slice(&peer.0);
    finish_icmpv6_checksum(unspecified, destination, &mut solicitation);
    if state.receive_icmpv6(unspecified, destination, &solicitation, 102)
        != Ipv6IngressEvent::NeighborSolicitation { return false; }
    state.dad.arm();
    state.receive_icmpv6(unspecified, destination, &solicitation, 103)
        == Ipv6IngressEvent::DuplicateAddress
}

#[cfg(feature = "stage14-2-test")]
const DHCPV6_OPTION_IA_NA: u16 = 3;
#[cfg(feature = "stage14-2-test")]
const DHCPV6_OPTION_IAADDR: u16 = 5;

#[cfg(feature = "stage14-2-test")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dhcpv6Lease {
    pub iaid: u32,
    pub address: Ipv6Address,
    pub preferred_until: u64,
    pub valid_until: u64,
    pub renew_at: u64,
    pub rebind_at: u64,
}

#[cfg(feature = "stage14-2-test")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dhcpv6LeaseState {
    Bound,
    Renewing,
    Rebinding,
    Expired,
}

#[cfg(feature = "stage14-2-test")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dhcpv6LeaseBinding {
    pub lease: Dhcpv6Lease,
    pub state: Dhcpv6LeaseState,
}

#[cfg(feature = "stage14-2-test")]
impl Dhcpv6LeaseBinding {
    pub fn advance(&mut self, now: u64) {
        self.state = if now >= self.lease.valid_until {
            Dhcpv6LeaseState::Expired
        } else if now >= self.lease.rebind_at {
            Dhcpv6LeaseState::Rebinding
        } else if now >= self.lease.renew_at {
            Dhcpv6LeaseState::Renewing
        } else {
            Dhcpv6LeaseState::Bound
        };
    }
}

#[cfg(feature = "stage14-2-test")]
fn parse_dhcpv6_lease(message: &Dhcpv6Message<'_>, now: u64) -> Option<Dhcpv6Lease> {
    let ia_na = dhcpv6_find_option(message, DHCPV6_OPTION_IA_NA)?;
    if ia_na.value.len() < 12 {
        return None;
    }
    let iaid = u32::from_be_bytes(ia_na.value[0..4].try_into().ok()?);
    let t1 = u32::from_be_bytes(ia_na.value[4..8].try_into().ok()?) as u64;
    let t2 = u32::from_be_bytes(ia_na.value[8..12].try_into().ok()?) as u64;
    if t1 == 0 || t2 == 0 || t1 > t2 {
        return None;
    }

    let mut nested = &ia_na.value[12..];
    while !nested.is_empty() {
        if nested.len() < 4 {
            return None;
        }
        let code = u16::from_be_bytes([nested[0], nested[1]]);
        let length = u16::from_be_bytes([nested[2], nested[3]]) as usize;
        let total = 4usize.checked_add(length)?;
        if total > nested.len() {
            return None;
        }
        if code == DHCPV6_OPTION_IAADDR {
            if length < 24 {
                return None;
            }
            let mut address = [0u8; 16];
            address.copy_from_slice(&nested[4..20]);
            let address = Ipv6Address(address);
            let preferred = u32::from_be_bytes(nested[20..24].try_into().ok()?) as u64;
            let valid = u32::from_be_bytes(nested[24..28].try_into().ok()?) as u64;
            if preferred > valid
                || valid == 0
                || address.is_multicast()
                || address.is_unspecified()
                || t2 > valid
            {
                return None;
            }
            return Some(Dhcpv6Lease {
                iaid,
                address,
                preferred_until: now.saturating_add(preferred),
                valid_until: now.saturating_add(valid),
                renew_at: now.saturating_add(t1),
                rebind_at: now.saturating_add(t2),
            });
        }
        nested = &nested[total..];
    }
    None
}

#[cfg(feature = "stage14-2-test")]
impl Dhcpv6Client {
    pub fn accept_reply_lease(
        &mut self,
        packet: &[u8],
        now: u64,
    ) -> Option<Dhcpv6LeaseBinding> {
        if self.state != Dhcpv6ClientState::Requesting {
            return None;
        }
        let message = parse_dhcpv6_message(packet)?;
        if message.message_type != Dhcpv6MessageType::Reply
            || message.transaction_id != self.transaction_id
            || !self.identifiers_match(&message)
        {
            return None;
        }
        let server = dhcpv6_find_option(&message, DHCPV6_OPTION_SERVER_ID)?;
        if self.server_id.is_none_or(|expected| expected.as_slice() != server.value) {
            return None;
        }
        let lease = parse_dhcpv6_lease(&message, now)?;
        self.state = Dhcpv6ClientState::Bound;
        self.retry_count = 0;
        self.next_retry_at = 0;
        Some(Dhcpv6LeaseBinding {
            lease,
            state: Dhcpv6LeaseState::Bound,
        })
    }
}

#[cfg(feature = "stage14-2-test")]
pub fn stage14_2_dhcpv6_lease_self_test() -> bool {
    let client_id = [0, 3, 0, 1, 0x02, 0, 0, 0, 0, 1];
    let server_id = [0, 3, 0, 1, 0x02, 0, 0, 0, 0, 2];
    let address = [0x20, 0x01, 0x0d, 0xb8, 0, 2, 0, 0, 0x02, 0, 0xff, 0xfe, 0, 0, 0, 9];
    let transaction_id = 0x0065_4321;

    let mut iaaddr = [0u8; 28];
    iaaddr[..2].copy_from_slice(&DHCPV6_OPTION_IAADDR.to_be_bytes());
    iaaddr[2..4].copy_from_slice(&24u16.to_be_bytes());
    iaaddr[4..20].copy_from_slice(&address);
    iaaddr[20..24].copy_from_slice(&60u32.to_be_bytes());
    iaaddr[24..28].copy_from_slice(&120u32.to_be_bytes());

    let mut ia_na_value = [0u8; 40];
    ia_na_value[..4].copy_from_slice(&7u32.to_be_bytes());
    ia_na_value[4..8].copy_from_slice(&30u32.to_be_bytes());
    ia_na_value[8..12].copy_from_slice(&90u32.to_be_bytes());
    ia_na_value[12..40].copy_from_slice(&iaaddr);

    let mut options = [0u8; 96];
    let Some(client_len) = write_dhcpv6_option(&mut options, DHCPV6_OPTION_CLIENT_ID, &client_id) else {
        return false;
    };
    let Some(server_len) = write_dhcpv6_option(
        &mut options[client_len..],
        DHCPV6_OPTION_SERVER_ID,
        &server_id,
    ) else {
        return false;
    };
    let Some(ia_len) = write_dhcpv6_option(
        &mut options[client_len + server_len..],
        DHCPV6_OPTION_IA_NA,
        &ia_na_value,
    ) else {
        return false;
    };
    let options_len = client_len + server_len + ia_len;

    let mut packet = [0u8; 128];
    let Some(reply_len) = write_dhcpv6_message(
        &mut packet,
        Dhcpv6MessageType::Reply,
        transaction_id,
        &options[..options_len],
    ) else {
        return false;
    };

    let Some(mut client) = Dhcpv6Client::new(transaction_id, &client_id) else {
        return false;
    };
    client.state = Dhcpv6ClientState::Requesting;
    client.server_id = Dhcpv6Duid::new(&server_id);
    let Some(mut binding) = client.accept_reply_lease(&packet[..reply_len], 100) else {
        return false;
    };
    if binding.lease.iaid != 7
        || binding.lease.address != Ipv6Address(address)
        || binding.lease.renew_at != 130
        || binding.lease.rebind_at != 190
        || binding.lease.preferred_until != 160
        || binding.lease.valid_until != 220
        || binding.state != Dhcpv6LeaseState::Bound
    {
        return false;
    }
    binding.advance(130);
    if binding.state != Dhcpv6LeaseState::Renewing {
        return false;
    }
    binding.advance(190);
    if binding.state != Dhcpv6LeaseState::Rebinding {
        return false;
    }
    binding.advance(220);
    if binding.state != Dhcpv6LeaseState::Expired {
        return false;
    }

    let mut invalid_ia = ia_na_value;
    invalid_ia[4..8].copy_from_slice(&100u32.to_be_bytes());
    invalid_ia[8..12].copy_from_slice(&90u32.to_be_bytes());
    let mut invalid_options = options;
    let ia_start = client_len + server_len;
    invalid_options[ia_start + 4..ia_start + 44].copy_from_slice(&invalid_ia);
    let Some(invalid_len) = write_dhcpv6_message(
        &mut packet,
        Dhcpv6MessageType::Reply,
        transaction_id,
        &invalid_options[..options_len],
    ) else {
        return false;
    };
    let Some(mut invalid_client) = Dhcpv6Client::new(transaction_id, &client_id) else {
        return false;
    };
    invalid_client.state = Dhcpv6ClientState::Requesting;
    invalid_client.server_id = Dhcpv6Duid::new(&server_id);
    invalid_client.accept_reply_lease(&packet[..invalid_len], 100).is_none()
}

pub struct VirtioSmolDevice {
    rx: [u8; MAX_FRAME],
}
impl VirtioSmolDevice {
    pub const fn new() -> Self {
        Self { rx: [0; MAX_FRAME] }
    }
}

pub struct WovenRxToken<'a> {
    data: &'a mut [u8],
}
pub struct WovenTxToken;

impl RxToken for WovenRxToken<'_> {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        f(self.data)
    }
}
impl TxToken for WovenTxToken {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut frame = [0u8; MAX_FRAME];
        let usable = core::cmp::min(len, MAX_FRAME);
        let result = f(&mut frame[..usable]);
        let _ = virtio_net::transmit(&frame[..usable]);
        result
    }
}

impl Device for VirtioSmolDevice {
    type RxToken<'a>
        = WovenRxToken<'a>
    where
        Self: 'a;
    type TxToken<'a>
        = WovenTxToken
    where
        Self: 'a;

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        let len = virtio_net::receive_into(&mut self.rx)?;
        Some((
            WovenRxToken {
                data: &mut self.rx[..len],
            },
            WovenTxToken,
        ))
    }
    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        // Returning Some(token) is a contract that the token can actually
        // submit a frame. The legacy virtio-net transport currently owns one
        // polling-mode TX descriptor, so defer smoltcp transmission while that
        // descriptor is still outstanding instead of handing out a token that
        // would silently drop the frame in `consume()`.
        virtio_net::tx_available().then_some(WovenTxToken)
    }
    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.medium = Medium::Ethernet;
        // VirtioSmolDevice currently submits complete Ethernet frames and does
        // not request or implement virtio-net checksum offload.  Therefore
        // smoltcp must compute/verify IPv4, ICMP, UDP and TCP checksums in
        // software.  `ChecksumCapabilities::ignored()` disables that work and
        // produces invalid live-network traffic (most visibly DHCP timeouts).
        //
        // For Ethernet devices smoltcp expects this value to include the
        // 14-byte Ethernet header, so 1514 corresponds to the normal 1500-byte
        // IPv4 MTU.
        caps.max_transmission_unit = 1514;
        caps.checksum = ChecksumCapabilities::default();
        caps
    }
}

// The candidate Wi-Fi transport has the same feature boundary as its backend.
#[cfg(not(feature = "stage13-9-test"))]
type NetTransport = VirtioSmolDevice;
#[cfg(feature = "stage13-9-test")]
pub use wifi_transport::{NetTransport, WifiNetDevice};

#[cfg(feature = "stage13-9-test")]
mod wifi_transport {
    use super::*;
    use crate::{wifi_session::WifiSession, wifi_smol};

    pub struct WifiNetDevice {
        session: WifiSession,
        epoch: u32,
        rx: [u8; wifi_smol::ETHERNET_MTU],
    }
    impl WifiNetDevice {
        pub fn new(session: WifiSession, epoch: u32) -> Self {
            Self {
                session,
                epoch,
                rx: [0; wifi_smol::ETHERNET_MTU],
            }
        }
        #[cfg(feature = "stage13-9-test")]
        pub fn session_mut(&mut self) -> &mut WifiSession {
            &mut self.session
        }
    }

    #[allow(clippy::large_enum_variant)]
    pub enum NetTransport {
        Virtio(VirtioSmolDevice),
        Wifi(WifiNetDevice),
    }
    pub enum NetRxToken<'a> {
        Virtio(WovenRxToken<'a>),
        Wifi(wifi_smol::WifiRxToken<'a>),
    }
    pub enum NetTxToken<'a> {
        Virtio(WovenTxToken),
        Wifi(wifi_smol::WifiTxToken<'a>),
    }
    impl RxToken for NetRxToken<'_> {
        fn consume<R, F>(self, f: F) -> R
        where
            F: FnOnce(&[u8]) -> R,
        {
            match self {
                Self::Virtio(t) => t.consume(f),
                Self::Wifi(t) => t.consume(f),
            }
        }
    }
    impl TxToken for NetTxToken<'_> {
        fn consume<R, F>(self, len: usize, f: F) -> R
        where
            F: FnOnce(&mut [u8]) -> R,
        {
            match self {
                Self::Virtio(t) => t.consume(len, f),
                Self::Wifi(t) => t.consume(len, f),
            }
        }
    }
    impl Device for NetTransport {
        type RxToken<'a>
            = NetRxToken<'a>
        where
            Self: 'a;
        type TxToken<'a>
            = NetTxToken<'a>
        where
            Self: 'a;

        fn receive(
            &mut self,
            timestamp: Instant,
        ) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
            match self {
                Self::Virtio(d) => d
                    .receive(timestamp)
                    .map(|(r, t)| (NetRxToken::Virtio(r), NetTxToken::Virtio(t))),
                Self::Wifi(d) => {
                    let len = match d.session.receive_ethernet(d.epoch, &mut d.rx) {
                        Ok(Some(n)) => n,
                        Ok(None) | Err(_) => return None,
                    };
                    Some((
                        NetRxToken::Wifi(wifi_smol::WifiRxToken::new(&mut d.rx[..len])),
                        NetTxToken::Wifi(wifi_smol::WifiTxToken::new(&mut d.session, d.epoch)),
                    ))
                }
            }
        }
        fn transmit(&mut self, timestamp: Instant) -> Option<Self::TxToken<'_>> {
            match self {
                Self::Virtio(d) => d.transmit(timestamp).map(NetTxToken::Virtio),
                Self::Wifi(d) => d
                    .session
                    .is_active_epoch(d.epoch)
                    .then_some(NetTxToken::Wifi(wifi_smol::WifiTxToken::new(
                        &mut d.session,
                        d.epoch,
                    ))),
            }
        }
        fn capabilities(&self) -> DeviceCapabilities {
            match self {
                Self::Virtio(d) => d.capabilities(),
                Self::Wifi(_) => {
                    let mut c = DeviceCapabilities::default();
                    c.medium = Medium::Ethernet;
                    c.max_transmission_unit = wifi_smol::ETHERNET_MTU;
                    c.checksum = ChecksumCapabilities::default();
                    c
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SocketKind {
    Udp = 1,
    Tcp = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SocketError {
    Offline,
    Invalid,
    NoSlot,
    WrongOwner,
    WrongKind,
    NotBound,
    NotConnected,
    WouldBlock,
    BufferFull,
    Address,
}

#[derive(Clone, Copy)]
struct UserSocket {
    owner: u64,
    handle: SocketHandle,
    kind: SocketKind,
    peer: Option<IpEndpoint>,
    generation: u32,
    async_refs: u16,
    closing: bool,
    close_started: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SocketToken {
    slot: u16,
    generation: u32,
    owner: u64,
}

#[derive(Clone, Copy)]
struct DnsQuerySlot {
    handle: dns::QueryHandle,
    generation: u32,
}

fn dns_token(slot: usize, generation: u32) -> u64 {
    (u64::from(generation) << 32) | slot as u64
}

fn decode_dns_token(token: u64) -> Result<(usize, u32), SocketError> {
    let slot = (token & 0xffff_ffff) as usize;
    let generation = (token >> 32) as u32;
    if slot >= 4 || generation == 0 {
        return Err(SocketError::Invalid);
    }
    Ok((slot, generation))
}

struct Runtime {
    iface: Interface,
    device: NetTransport,
    sockets: SocketSet<'static>,
    user: [Option<UserSocket>; MAX_USER_SOCKETS],
    #[cfg(feature = "stage10-7-test")]
    queued_close_verified: bool,
    echo_handle: Option<SocketHandle>,
    echo_port: u16,
    echo_packets: u64,
    dhcp_handle: Option<SocketHandle>,
    dns_handle: Option<SocketHandle>,
    dns_queries: [Option<DnsQuerySlot>; 4],
    dhcp_enabled: bool,
    using_dhcp: bool,
    ipv4: Ipv4Address,
    prefix: u8,
    gateway: Ipv4Address,
    dns_server: Ipv4Address,
    next_ephemeral: u16,
    next_socket_generation: u32,
    next_dns_generation: u32,
    ping_handle: Option<SocketHandle>,
    ping_pending: Option<(Ipv4Address, u16, u64)>,
    ping_sequence: u16,
}
static RUNTIME: Once<Mutex<Runtime>> = Once::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EchoError {
    NetworkOffline,
    AlreadyConfigured,
    BindFailed,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NetStats {
    pub online: bool,
    pub echo_active: bool,
    pub echo_port: u16,
    pub echo_packets: u64,
    pub user_sockets: usize,
    pub dhcp_enabled: bool,
    pub using_dhcp: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct NetInfo {
    pub ipv4: [u8; 4],
    pub gateway: [u8; 4],
    pub dns: [u8; 4],
    pub prefix: u8,
    pub dhcp_enabled: u8,
    pub using_dhcp: u8,
    pub online: u8,
    pub mac: [u8; 6],
    pub _reserved: [u8; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InitError {
    Transport(virtio_net::InitError),
    Route,
}

pub fn init() -> Result<(), InitError> {
    if RUNTIME.get().is_some() {
        return Ok(());
    }
    virtio_net::init().map_err(InitError::Transport)?;

    let mac_bytes = virtio_net::mac_address();
    let mac = EthernetAddress(mac_bytes);
    #[cfg(feature = "stage13-9-test")]
    let mut device = NetTransport::Virtio(VirtioSmolDevice::new());
    #[cfg(not(feature = "stage13-9-test"))]
    let mut device = VirtioSmolDevice::new();
    let mut config = Config::new(mac.into());
    // Previously a fixed constant (0x5748_4f53_4e45_5435), which made TCP
    // initial sequence numbers and smoltcp's internal randomized choices
    // predictable to a network attacker on every boot. See `entropy.rs`.
    config.random_seed = crate::entropy::random_u64();
    let mut iface = Interface::new(config, &mut device, now());
    iface.update_ip_addrs(|addrs| {
        let _ = addrs.push(default_cidr());
        #[cfg(feature = "stage14-2-test")]
        {
            let link_local = stage14_2_link_local_from_mac(mac_bytes);
            let _ = addrs.push(IpCidr::new(
                IpAddress::Ipv6(SmolIpv6Address::from_octets(link_local.0)),
                64,
            ));
        }
    });
    iface
        .routes_mut()
        .add_default_ipv4_route(DEFAULT_GATEWAY)
        .map_err(|_| InitError::Route)?;

    let mut sockets = SocketSet::new(Vec::new());

    // Stage 6 DHCP client. We keep the known-good QEMU static address until a
    // lease is actually acquired, so losing DHCP cannot take down recovery.
    let dhcp_handle = sockets.add(dhcpv4::Socket::new());

    // QEMU user networking provides DNS proxy 10.0.2.3. DNS queries are
    // asynchronous and may be started/polled by the userspace ABI.
    let dns_servers = [IpAddress::Ipv4(DEFAULT_DNS)];
    let dns_queries = (0..4).map(|_| None).collect::<Vec<_>>();
    let dns_handle = sockets.add(dns::Socket::new(&dns_servers, dns_queries));

    let ping_rx = icmp::PacketBuffer::new(vec![icmp::PacketMetadata::EMPTY; 4], vec![0; 1024]);
    let ping_tx = icmp::PacketBuffer::new(vec![icmp::PacketMetadata::EMPTY; 4], vec![0; 1024]);
    let mut ping_socket = icmp::Socket::new(ping_rx, ping_tx);
    ping_socket
        .bind(icmp::Endpoint::Ident(0x5748))
        .map_err(|_| InitError::Route)?;
    let ping_handle = sockets.add(ping_socket);

    RUNTIME.call_once(|| {
        Mutex::with_rank(
            Runtime {
                iface,
                device,
                sockets,
                user: [None; MAX_USER_SOCKETS],
                #[cfg(feature = "stage10-7-test")]
                queued_close_verified: false,
                echo_handle: None,
                echo_port: 0,
                echo_packets: 0,
                dhcp_handle: Some(dhcp_handle),
                dns_handle: Some(dns_handle),
                dns_queries: [None; 4],
                dhcp_enabled: true,
                using_dhcp: false,
                ipv4: DEFAULT_IPV4,
                prefix: DEFAULT_PREFIX,
                gateway: DEFAULT_GATEWAY,
                dns_server: DEFAULT_DNS,
                next_ephemeral: 49152,
                next_socket_generation: 1,
                next_dns_generation: 1,
                ping_handle: Some(ping_handle),
                ping_pending: None,
                ping_sequence: 0,
            },
            20,
        )
    });
    Ok(())
}

pub fn poll() {
    virtio_net::poll();
    let Some(runtime) = RUNTIME.get() else {
        return;
    };
    let mut runtime = runtime.lock();

    // Force the Stage 10.7 close-before-poll interleaving: the first TCP
    // payload cannot leave until the final pin is released on a closed fd.
    #[cfg(feature = "stage10-7-test")]
    if !runtime.queued_close_verified
        && runtime.user.iter().flatten().any(|entry| {
            entry.kind == SocketKind::Tcp
                && runtime
                    .sockets
                    .get::<tcp::Socket>(entry.handle)
                    .send_queue()
                    != 0
        })
    {
        return;
    }

    {
        let Runtime {
            iface,
            device,
            sockets,
            ..
        } = &mut *runtime;
        let _ = iface.poll(now(), device, sockets);
    }

    // Apply DHCP only after a valid lease arrives. Deconfiguration falls back
    // to the static QEMU topology so the diagnostic shell stays reachable.
    if runtime.dhcp_enabled {
        if let Some(handle) = runtime.dhcp_handle {
            // Copy lease data out of the DHCP socket before mutating the rest
            // of Runtime. This keeps the SocketSet mutable borrow tightly scoped.
            let update = {
                let socket = runtime.sockets.get_mut::<dhcpv4::Socket>(handle);
                match socket.poll() {
                    Some(dhcpv4::Event::Configured(config)) => {
                        let dns = config.dns_servers.first().copied().unwrap_or(DEFAULT_DNS);
                        Some(Some((
                            config.address,
                            config.router.unwrap_or(DEFAULT_GATEWAY),
                            dns,
                        )))
                    }
                    Some(dhcpv4::Event::Deconfigured) => Some(None),
                    None => None,
                }
            };

            match update {
                Some(Some((address, router, dns))) => {
                    apply_dhcp_locked(&mut runtime, address, router, dns);
                }
                Some(None) if runtime.using_dhcp => {
                    apply_static_locked(&mut runtime);
                }
                Some(None) | None => {}
            }
        }
    }

    if let Some(handle) = runtime.echo_handle {
        let mut reply = [0u8; 512];
        let received = {
            let socket = runtime.sockets.get_mut::<udp::Socket>(handle);
            socket.recv_slice(&mut reply).ok()
        };
        if let Some((len, remote)) = received {
            if runtime
                .sockets
                .get_mut::<udp::Socket>(handle)
                .send_slice(&reply[..len], remote)
                .is_ok()
            {
                runtime.echo_packets = runtime.echo_packets.saturating_add(1);
            }
        }
    }

    {
        let Runtime {
            iface,
            device,
            sockets,
            ..
        } = &mut *runtime;
        let _ = iface.poll(now(), device, sockets);
    }
    for index in 0..MAX_USER_SOCKETS {
        retire_closed_socket(&mut runtime, index);
    }
    drop(runtime);
    crate::async_network::network_progress();
}

/// Caller holds the runtime lock. Descriptor revocation is immediate, but
/// queued TCP bytes and FIN need a live transport until close completes.
/// Existing async references retain the socket even during owner teardown.
fn retire_closed_socket(runtime: &mut Runtime, index: usize) {
    let Some(entry) = runtime.user[index] else {
        return;
    };
    if !entry.closing || entry.async_refs != 0 {
        return;
    }
    if entry.kind == SocketKind::Tcp {
        let socket = runtime.sockets.get_mut::<tcp::Socket>(entry.handle);
        if entry.close_started.is_none() {
            #[cfg(feature = "stage10-7-test")]
            crate::serial::write_line(format_args!(
                "[S10.7-DIAG] graceful TCP close tx_queue={}",
                socket.send_queue()
            ));
            socket.close();
            runtime.user[index].as_mut().unwrap().close_started = Some(timer::ticks());
        }
        if socket.is_open() {
            let started = runtime.user[index].unwrap().close_started.unwrap();
            if timer::ticks().wrapping_sub(started) < TCP_CLOSE_GRACE_TICKS {
                return;
            }
            // A dead peer cannot retain one of the bounded slots forever.
            // No async token remains; expiration deliberately abandons drain.
            socket.abort();
        }
    }
    let _ = runtime.sockets.remove(entry.handle);
    runtime.user[index] = None;
}

fn cancel_dns_queries_locked(runtime: &mut Runtime) {
    let Some(handle) = runtime.dns_handle else {
        runtime.dns_queries.fill(None);
        return;
    };
    for slot in &mut runtime.dns_queries {
        if let Some(query) = slot.take() {
            runtime
                .sockets
                .get_mut::<dns::Socket>(handle)
                .cancel_query(query.handle);
        }
    }
}

fn apply_dhcp_locked(
    runtime: &mut Runtime,
    address: smoltcp::wire::Ipv4Cidr,
    router: Ipv4Address,
    dns: Ipv4Address,
) {
    let resolver_changed = runtime.dns_server != dns;
    if resolver_changed {
        cancel_dns_queries_locked(runtime);
    }
    runtime.iface.update_ip_addrs(|addrs| {
        addrs.clear();
        let _ = addrs.push(IpCidr::Ipv4(address));
    });
    runtime.iface.routes_mut().remove_default_ipv4_route();
    let _ = runtime.iface.routes_mut().add_default_ipv4_route(router);
    runtime.ipv4 = address.address();
    runtime.prefix = address.prefix_len();
    runtime.gateway = router;
    runtime.dns_server = dns;
    runtime.using_dhcp = true;
    if resolver_changed {
        if let Some(handle) = runtime.dns_handle {
            runtime
                .sockets
                .get_mut::<dns::Socket>(handle)
                .update_servers(&[IpAddress::Ipv4(dns)]);
        }
    }
}

fn apply_static_locked(runtime: &mut Runtime) {
    let resolver_changed = runtime.dns_server != DEFAULT_DNS;
    if resolver_changed {
        cancel_dns_queries_locked(runtime);
    }
    runtime.iface.update_ip_addrs(|addrs| {
        addrs.clear();
        let _ = addrs.push(default_cidr());
    });
    runtime.iface.routes_mut().remove_default_ipv4_route();
    let _ = runtime
        .iface
        .routes_mut()
        .add_default_ipv4_route(DEFAULT_GATEWAY);
    runtime.ipv4 = DEFAULT_IPV4;
    runtime.prefix = DEFAULT_PREFIX;
    runtime.gateway = DEFAULT_GATEWAY;
    runtime.dns_server = DEFAULT_DNS;
    runtime.using_dhcp = false;
    if resolver_changed {
        if let Some(handle) = runtime.dns_handle {
            runtime
                .sockets
                .get_mut::<dns::Socket>(handle)
                .update_servers(&[IpAddress::Ipv4(DEFAULT_DNS)]);
        }
    }
}

pub fn set_dhcp(enabled: bool) -> Result<(), SocketError> {
    let Some(runtime) = RUNTIME.get() else {
        return Err(SocketError::Offline);
    };
    let mut runtime = runtime.lock();
    runtime.dhcp_enabled = enabled;
    if !enabled {
        if let Some(handle) = runtime.dhcp_handle {
            runtime.sockets.get_mut::<dhcpv4::Socket>(handle).reset();
        }
        apply_static_locked(&mut runtime);
    }
    Ok(())
}

pub fn start_udp_echo(port: u16) -> Result<(), EchoError> {
    if port == 0 {
        return Err(EchoError::BindFailed);
    }
    let Some(runtime) = RUNTIME.get() else {
        return Err(EchoError::NetworkOffline);
    };
    let mut runtime = runtime.lock();
    if runtime.echo_handle.is_some() {
        return if runtime.echo_port == port {
            Ok(())
        } else {
            Err(EchoError::AlreadyConfigured)
        };
    }
    let rx = udp::PacketBuffer::new(
        vec![udp::PacketMetadata::EMPTY; UDP_META_SLOTS],
        vec![0; SOCKET_BUFFER_BYTES],
    );
    let tx = udp::PacketBuffer::new(
        vec![udp::PacketMetadata::EMPTY; UDP_META_SLOTS],
        vec![0; SOCKET_BUFFER_BYTES],
    );
    let mut socket = udp::Socket::new(rx, tx);
    socket.bind(port).map_err(|_| EchoError::BindFailed)?;
    let handle = runtime.sockets.add(socket);
    runtime.echo_handle = Some(handle);
    runtime.echo_port = port;
    Ok(())
}

fn find_slot(runtime: &Runtime, owner: u64, id: u64) -> Result<UserSocket, SocketError> {
    let index = usize::try_from(id).map_err(|_| SocketError::Invalid)?;
    let socket = runtime
        .user
        .get(index)
        .and_then(|slot| *slot)
        .ok_or(SocketError::Invalid)?;
    if socket.owner != owner {
        return Err(SocketError::WrongOwner);
    }
    if socket.closing {
        return Err(SocketError::Invalid);
    }
    Ok(socket)
}

fn next_ephemeral(runtime: &mut Runtime) -> u16 {
    let port = runtime.next_ephemeral;
    runtime.next_ephemeral = if port >= 65534 { 49152 } else { port + 1 };
    port
}

pub fn socket_open(owner: u64, kind: SocketKind) -> Result<u64, SocketError> {
    let Some(runtime) = RUNTIME.get() else {
        return Err(SocketError::Offline);
    };
    let mut runtime = runtime.lock();
    let slot = runtime
        .user
        .iter()
        .position(Option::is_none)
        .ok_or(SocketError::NoSlot)?;
    let handle = match kind {
        SocketKind::Udp => {
            let rx = udp::PacketBuffer::new(
                vec![udp::PacketMetadata::EMPTY; UDP_META_SLOTS],
                vec![0; SOCKET_BUFFER_BYTES],
            );
            let tx = udp::PacketBuffer::new(
                vec![udp::PacketMetadata::EMPTY; UDP_META_SLOTS],
                vec![0; SOCKET_BUFFER_BYTES],
            );
            runtime.sockets.add(udp::Socket::new(rx, tx))
        }
        SocketKind::Tcp => {
            let rx = tcp::SocketBuffer::new(vec![0; SOCKET_BUFFER_BYTES]);
            let tx = tcp::SocketBuffer::new(vec![0; SOCKET_BUFFER_BYTES]);
            runtime.sockets.add(tcp::Socket::new(rx, tx))
        }
    };
    let generation = runtime.next_socket_generation.max(1);
    runtime.next_socket_generation = generation.wrapping_add(1).max(1);
    runtime.user[slot] = Some(UserSocket {
        owner,
        handle,
        kind,
        peer: None,
        generation,
        async_refs: 0,
        closing: false,
        close_started: None,
    });
    Ok(slot as u64)
}

pub fn socket_bind(owner: u64, id: u64, port: u16) -> Result<(), SocketError> {
    if port == 0 {
        return Err(SocketError::Address);
    }
    let Some(runtime) = RUNTIME.get() else {
        return Err(SocketError::Offline);
    };
    let mut runtime = runtime.lock();
    let entry = find_slot(&runtime, owner, id)?;
    match entry.kind {
        SocketKind::Udp => runtime
            .sockets
            .get_mut::<udp::Socket>(entry.handle)
            .bind(port)
            .map_err(|_| SocketError::Address),
        SocketKind::Tcp => runtime
            .sockets
            .get_mut::<tcp::Socket>(entry.handle)
            .listen(port)
            .map_err(|_| SocketError::Address),
    }
}

#[cfg(any(feature = "stage14-3-test", feature = "stage14-4-test"))]
pub fn socket_connect_v1(
    owner: u64,
    id: u64,
    endpoint: SocketEndpointV1,
) -> Result<(), SocketError> {
    socket_connect(owner, id, endpoint.to_endpoint()?)
}

#[cfg(any(feature = "stage14-3-test", feature = "stage14-4-test"))]
pub fn socket_peer_v1(owner: u64, id: u64) -> Result<Option<SocketEndpointV1>, SocketError> {
    socket_peer(owner, id).map(|peer| peer.map(SocketEndpointV1::from_endpoint))
}

pub fn socket_connect(owner: u64, id: u64, endpoint: IpEndpoint) -> Result<(), SocketError> {
    if !valid_socket_endpoint(endpoint) {
        return Err(SocketError::Address);
    }
    let Some(runtime) = RUNTIME.get() else {
        return Err(SocketError::Offline);
    };
    let mut runtime = runtime.lock();
    let entry = find_slot(&runtime, owner, id)?;
    match entry.kind {
        SocketKind::Udp => {
            // A connected UDP socket needs a local endpoint before smoltcp can
            // transmit. Match normal socket semantics by assigning an
            // ephemeral port when userspace connects an unbound datagram
            // socket. Without this, send_slice reports an addressing failure;
            // treating that permanent condition as WouldBlock would make an
            // asynchronous sender retry forever.
            let needs_bind = !runtime.sockets.get::<udp::Socket>(entry.handle).is_open();
            if needs_bind {
                let mut bound = false;
                for _ in 0..16 {
                    let local_port = next_ephemeral(&mut runtime);
                    if runtime
                        .sockets
                        .get_mut::<udp::Socket>(entry.handle)
                        .bind(local_port)
                        .is_ok()
                    {
                        bound = true;
                        break;
                    }
                }
                if !bound {
                    return Err(SocketError::Address);
                }
            }
            runtime.user[id as usize].as_mut().unwrap().peer = Some(endpoint);
            Ok(())
        }
        SocketKind::Tcp => {
            let local_port = next_ephemeral(&mut runtime);
            let Runtime { iface, sockets, .. } = &mut *runtime;
            sockets
                .get_mut::<tcp::Socket>(entry.handle)
                .connect(iface.context(), endpoint, local_port)
                .map_err(|_| SocketError::Address)?;
            runtime.user[id as usize].as_mut().unwrap().peer = Some(endpoint);
            Ok(())
        }
    }
}

pub fn socket_send(owner: u64, id: u64, data: &[u8]) -> Result<usize, SocketError> {
    let Some(runtime) = RUNTIME.get() else {
        return Err(SocketError::Offline);
    };
    let mut runtime = runtime.lock();
    let entry = find_slot(&runtime, owner, id)?;
    match entry.kind {
        SocketKind::Udp => {
            let peer = entry.peer.ok_or(SocketError::NotConnected)?;
            runtime
                .sockets
                .get_mut::<udp::Socket>(entry.handle)
                .send_slice(data, peer)
                .map(|()| data.len())
                .map_err(|_| SocketError::BufferFull)
        }
        SocketKind::Tcp => runtime
            .sockets
            .get_mut::<tcp::Socket>(entry.handle)
            .send_slice(data)
            .map_err(|_| SocketError::WouldBlock),
    }
}

pub fn socket_recv(
    owner: u64,
    id: u64,
    out: &mut [u8],
) -> Result<(usize, Option<IpEndpoint>), SocketError> {
    let Some(runtime) = RUNTIME.get() else {
        return Err(SocketError::Offline);
    };
    let mut runtime = runtime.lock();
    let entry = find_slot(&runtime, owner, id)?;
    match entry.kind {
        SocketKind::Udp => runtime
            .sockets
            .get_mut::<udp::Socket>(entry.handle)
            .recv_slice(out)
            .map(|(len, meta)| (len, Some(meta.endpoint)))
            .map_err(|_| SocketError::WouldBlock),
        SocketKind::Tcp => runtime
            .sockets
            .get_mut::<tcp::Socket>(entry.handle)
            .recv_slice(out)
            .map(|len| (len, entry.peer))
            .map_err(|_| SocketError::WouldBlock),
    }
}

pub fn socket_close(owner: u64, id: u64) -> Result<(), SocketError> {
    let Some(runtime) = RUNTIME.get() else {
        return Err(SocketError::Offline);
    };
    let mut runtime = runtime.lock();
    let _ = find_slot(&runtime, owner, id)?;
    let index = id as usize;
    if let Some(socket) = runtime.user[index].as_mut() {
        socket.closing = true;
    }
    retire_closed_socket(&mut runtime, index);
    Ok(())
}

pub fn close_process_sockets(owner: u64) {
    let Some(runtime) = RUNTIME.get() else {
        return;
    };
    let mut runtime = runtime.lock();
    for index in 0..MAX_USER_SOCKETS {
        if let Some(entry) = runtime.user[index] {
            if entry.owner == owner {
                if let Some(socket) = runtime.user[index].as_mut() {
                    socket.closing = true;
                }
                retire_closed_socket(&mut runtime, index);
            }
        }
    }
}

pub fn pin_socket(owner: u64, id: u64) -> Result<SocketToken, SocketError> {
    let Some(runtime) = RUNTIME.get() else {
        return Err(SocketError::Offline);
    };
    let mut runtime = runtime.lock();
    let entry = find_slot(&runtime, owner, id)?;
    let index = usize::try_from(id).map_err(|_| SocketError::Invalid)?;
    let Some(socket) = runtime.user.get_mut(index).and_then(Option::as_mut) else {
        return Err(SocketError::Invalid);
    };
    socket.async_refs = socket
        .async_refs
        .checked_add(1)
        .ok_or(SocketError::BufferFull)?;
    Ok(SocketToken {
        slot: index as u16,
        generation: entry.generation,
        owner,
    })
}

fn find_token(runtime: &Runtime, token: SocketToken) -> Result<UserSocket, SocketError> {
    let socket = runtime
        .user
        .get(token.slot as usize)
        .and_then(|s| *s)
        .ok_or(SocketError::Invalid)?;
    if socket.owner != token.owner || socket.generation != token.generation {
        return Err(SocketError::Invalid);
    }
    Ok(socket)
}

pub fn unpin_socket(token: SocketToken) {
    let Some(runtime) = RUNTIME.get() else {
        return;
    };
    let mut runtime = runtime.lock();
    let index = token.slot as usize;
    let Some(current) = runtime.user.get(index).and_then(|s| *s) else {
        return;
    };
    if current.owner != token.owner || current.generation != token.generation {
        return;
    }
    if let Some(socket) = runtime.user[index].as_mut() {
        socket.async_refs = socket.async_refs.saturating_sub(1);
    }
    finish_close(&mut runtime, index);
}

// Keep the original bounded slot and generation until queued output drains.
// No blocking or scheduling occurs under the runtime lock. The normal network
// poll services these sockets even after their process has exited. A peer that
// never acknowledges cannot retain a slot forever (30-second drain bound).
fn finish_close(runtime: &mut Runtime, index: usize) {
    let Some(entry) = runtime.user[index] else {
        return;
    };
    if !entry.closing || entry.async_refs != 0 {
        return;
    }
    let _pending = match entry.kind {
        SocketKind::Tcp => {
            let socket = runtime.sockets.get_mut::<tcp::Socket>(entry.handle);
            let pending = socket.is_open() && socket.send_queue() != 0;
            #[cfg(feature = "stage10-7-test")]
            if pending {
                runtime.queued_close_verified = true;
            }
            socket.close();
            pending
        }
        SocketKind::Udp => false,
    };
    retire_closed_socket(runtime, index);
}

#[cfg(feature = "stage10-7-test")]
pub fn queued_close_verified() -> bool {
    RUNTIME
        .get()
        .is_some_and(|runtime| runtime.lock().queued_close_verified)
}

pub fn socket_connect_pinned(token: SocketToken, endpoint: IpEndpoint) -> Result<(), SocketError> {
    if !valid_socket_endpoint(endpoint) {
        return Err(SocketError::Address);
    }
    let Some(runtime) = RUNTIME.get() else {
        return Err(SocketError::Offline);
    };
    let mut runtime = runtime.lock();
    let entry = find_token(&runtime, token)?;
    if entry.kind != SocketKind::Tcp {
        return Err(SocketError::WrongKind);
    }

    if let Some(peer) = entry.peer {
        if peer != endpoint {
            return Err(SocketError::Address);
        }
    } else {
        let local_port = next_ephemeral(&mut runtime);
        let Runtime {
            iface,
            sockets,
            user,
            ..
        } = &mut *runtime;
        sockets
            .get_mut::<tcp::Socket>(entry.handle)
            .connect(iface.context(), endpoint, local_port)
            .map_err(|_| SocketError::Address)?;
        if let Some(socket) = user[token.slot as usize].as_mut() {
            socket.peer = Some(endpoint);
        }
    }

    let socket = runtime.sockets.get::<tcp::Socket>(entry.handle);
    if socket.may_send() {
        return Ok(());
    }
    if socket.is_active() {
        return Err(SocketError::WouldBlock);
    }
    Err(SocketError::NotConnected)
}

pub fn socket_send_pinned(token: SocketToken, data: &[u8]) -> Result<usize, SocketError> {
    let Some(runtime) = RUNTIME.get() else {
        return Err(SocketError::Offline);
    };
    let mut runtime = runtime.lock();
    let entry = find_token(&runtime, token)?;
    match entry.kind {
        SocketKind::Udp => {
            let peer = entry.peer.ok_or(SocketError::NotConnected)?;
            let socket = runtime.sockets.get_mut::<udp::Socket>(entry.handle);
            if !socket.is_open() {
                return Err(SocketError::Address);
            }
            socket
                .send_slice(data, peer)
                .map(|()| data.len())
                .map_err(|_| SocketError::WouldBlock)
        }
        SocketKind::Tcp => {
            let socket = runtime.sockets.get_mut::<tcp::Socket>(entry.handle);
            if !socket.is_active() {
                return Err(SocketError::NotConnected);
            }
            if !socket.may_send() {
                return Err(SocketError::NotConnected);
            }
            if !socket.can_send() {
                return Err(SocketError::WouldBlock);
            }
            socket.send_slice(data).map_err(|_| SocketError::WouldBlock)
        }
    }
}

pub fn socket_recv_pinned(
    token: SocketToken,
    out: &mut [u8],
) -> Result<(usize, Option<IpEndpoint>), SocketError> {
    let Some(runtime) = RUNTIME.get() else {
        return Err(SocketError::Offline);
    };
    let mut runtime = runtime.lock();
    let entry = find_token(&runtime, token)?;
    match entry.kind {
        SocketKind::Udp => {
            let (len, endpoint) = runtime
                .sockets
                .get_mut::<udp::Socket>(entry.handle)
                .recv_slice(out)
                .map(|(len, meta)| (len, meta.endpoint))
                .map_err(|_| SocketError::WouldBlock)?;
            if let Some(socket) = runtime.user[token.slot as usize].as_mut() {
                socket.peer = Some(endpoint);
            }
            Ok((len, Some(endpoint)))
        }
        SocketKind::Tcp => {
            let socket = runtime.sockets.get_mut::<tcp::Socket>(entry.handle);
            if socket.can_recv() {
                return socket
                    .recv_slice(out)
                    .map(|len| (len, entry.peer))
                    .map_err(|_| SocketError::WouldBlock);
            }
            if !socket.may_recv() {
                return Ok((0, entry.peer));
            }
            Err(SocketError::WouldBlock)
        }
    }
}

pub fn socket_peer(owner: u64, id: u64) -> Result<Option<IpEndpoint>, SocketError> {
    let Some(runtime) = RUNTIME.get() else {
        return Err(SocketError::Offline);
    };
    let runtime = runtime.lock();
    Ok(find_slot(&runtime, owner, id)?.peer)
}

pub fn net_info() -> NetInfo {
    if let Some(runtime) = RUNTIME.get() {
        let runtime = runtime.lock();
        NetInfo {
            ipv4: runtime.ipv4.octets(),
            gateway: runtime.gateway.octets(),
            dns: runtime.dns_server.octets(),
            prefix: runtime.prefix,
            dhcp_enabled: u8::from(runtime.dhcp_enabled),
            using_dhcp: u8::from(runtime.using_dhcp),
            online: u8::from(virtio_net::is_initialized()),
            mac: virtio_net::mac_address(),
            _reserved: [0; 2],
        }
    } else {
        NetInfo {
            ipv4: [0; 4],
            gateway: [0; 4],
            dns: [0; 4],
            prefix: 0,
            dhcp_enabled: 0,
            using_dhcp: 0,
            online: 0,
            mac: [0; 6],
            _reserved: [0; 2],
        }
    }
}

pub fn dns_start(name: &str) -> Result<u64, SocketError> {
    let Some(runtime) = RUNTIME.get() else {
        return Err(SocketError::Offline);
    };
    let mut runtime = runtime.lock();
    let handle = runtime.dns_handle.ok_or(SocketError::Offline)?;
    let slot = runtime
        .dns_queries
        .iter()
        .position(Option::is_none)
        .ok_or(SocketError::NoSlot)?;
    let query = {
        let Runtime { iface, sockets, .. } = &mut *runtime;
        sockets
            .get_mut::<dns::Socket>(handle)
            .start_query(iface.context(), name, smoltcp::wire::DnsQueryType::A)
            .map_err(|_| SocketError::BufferFull)?
    };
    let generation = runtime.next_dns_generation.max(1);
    runtime.next_dns_generation = generation.wrapping_add(1).max(1);
    runtime.dns_queries[slot] = Some(DnsQuerySlot {
        handle: query,
        generation,
    });
    Ok(dns_token(slot, generation))
}

pub fn dns_cancel(id: u64) -> Result<(), SocketError> {
    let Some(runtime) = RUNTIME.get() else {
        return Err(SocketError::Offline);
    };
    let mut runtime = runtime.lock();
    let (index, generation) = decode_dns_token(id)?;
    let query = runtime
        .dns_queries
        .get(index)
        .and_then(|slot| *slot)
        .filter(|slot| slot.generation == generation)
        .ok_or(SocketError::Invalid)?;
    runtime.dns_queries[index] = None;
    let handle = runtime.dns_handle.ok_or(SocketError::Offline)?;
    runtime
        .sockets
        .get_mut::<dns::Socket>(handle)
        .cancel_query(query.handle);
    Ok(())
}

pub fn dns_poll(id: u64) -> Result<Option<Ipv4Address>, SocketError> {
    let Some(runtime) = RUNTIME.get() else {
        return Err(SocketError::Offline);
    };
    let mut runtime = runtime.lock();
    let (index, generation) = decode_dns_token(id)?;
    let query = runtime
        .dns_queries
        .get(index)
        .and_then(|slot| *slot)
        .filter(|slot| slot.generation == generation)
        .ok_or(SocketError::Invalid)?;
    let handle = runtime.dns_handle.ok_or(SocketError::Offline)?;
    match runtime
        .sockets
        .get_mut::<dns::Socket>(handle)
        .get_query_result(query.handle)
    {
        Ok(addrs) => {
            runtime.dns_queries[index] = None;
            Ok(addrs.into_iter().next().and_then(|addr| match addr {
                IpAddress::Ipv4(v4) => Some(v4),
                IpAddress::Ipv6(_) => None,
            }))
        }
        Err(dns::GetQueryResultError::Pending) => Ok(None),
        Err(_) => {
            runtime.dns_queries[index] = None;
            Err(SocketError::Address)
        }
    }
}

pub fn ping_start(ip: Ipv4Address) -> Result<(), SocketError> {
    let Some(runtime) = RUNTIME.get() else {
        return Err(SocketError::Offline);
    };
    let mut runtime = runtime.lock();
    if runtime.ping_pending.is_some() {
        return Err(SocketError::WouldBlock);
    }
    let handle = runtime.ping_handle.ok_or(SocketError::Offline)?;
    runtime.ping_sequence = runtime.ping_sequence.wrapping_add(1);
    let seq = runtime.ping_sequence;
    let mut packet = [0u8; 25];
    packet[0] = 8; // ICMPv4 echo request
    packet[1] = 0;
    packet[4..6].copy_from_slice(&0x5748u16.to_be_bytes());
    packet[6..8].copy_from_slice(&seq.to_be_bytes());
    packet[8..].copy_from_slice(b"WovenHatStage5!!!");
    let checksum = internet_checksum(&packet);
    packet[2..4].copy_from_slice(&checksum.to_be_bytes());
    runtime
        .sockets
        .get_mut::<icmp::Socket>(handle)
        .send_slice(&packet, IpAddress::Ipv4(ip))
        .map_err(|_| SocketError::BufferFull)?;
    runtime.ping_pending = Some((ip, seq, timer::ticks()));
    Ok(())
}

/// Returns `Ok(None)` while awaiting a reply and `Ok(Some(rtt_ticks))` once
/// the matching echo reply arrives.
pub fn ping_poll() -> Result<Option<u64>, SocketError> {
    let Some(runtime) = RUNTIME.get() else {
        return Err(SocketError::Offline);
    };
    let mut runtime = runtime.lock();
    let Some((target, seq, started)) = runtime.ping_pending else {
        return Err(SocketError::Invalid);
    };
    if timer::ticks().saturating_sub(started) >= u64::from(timer::FREQUENCY_HZ) * 5 {
        runtime.ping_pending = None;
        return Err(SocketError::WouldBlock);
    }
    let handle = runtime.ping_handle.ok_or(SocketError::Offline)?;
    let mut packet = [0u8; 256];
    match runtime
        .sockets
        .get_mut::<icmp::Socket>(handle)
        .recv_slice(&mut packet)
    {
        Ok((len, source)) => {
            if len >= 8
                && source == IpAddress::Ipv4(target)
                && packet[0] == 0
                && u16::from_be_bytes([packet[4], packet[5]]) == 0x5748
                && u16::from_be_bytes([packet[6], packet[7]]) == seq
            {
                runtime.ping_pending = None;
                Ok(Some(timer::ticks().saturating_sub(started)))
            } else {
                Ok(None)
            }
        }
        Err(_) => Ok(None),
    }
}

fn internet_checksum(data: &[u8]) -> u16 {
    let mut sum = 0u32;
    let mut i = 0usize;
    while i + 1 < data.len() {
        sum = sum.wrapping_add(u16::from_be_bytes([data[i], data[i + 1]]) as u32);
        i += 2;
    }
    if i < data.len() {
        sum = sum.wrapping_add((data[i] as u32) << 8);
    }
    while (sum >> 16) != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

pub fn endpoint_from_packed(value: u64) -> Result<IpEndpoint, SocketError> {
    let ip = (value & 0xffff_ffff) as u32;
    let port = ((value >> 32) & 0xffff) as u16;
    let endpoint = IpEndpoint::new(
        IpAddress::Ipv4(Ipv4Address::from_octets(ip.to_be_bytes())),
        port,
    );
    valid_socket_endpoint(endpoint).then_some(endpoint).ok_or(SocketError::Address)
}

fn valid_socket_endpoint(endpoint: IpEndpoint) -> bool {
    if endpoint.port == 0 {
        return false;
    }
    match endpoint.addr {
        IpAddress::Ipv4(address) => {
            let octets = address.octets();
            octets != [0; 4] && !(224..=239).contains(&octets[0])
        }
        IpAddress::Ipv6(address) => !address.is_unspecified() && !address.is_multicast(),
    }
}

#[cfg(any(feature = "stage14-3-test", feature = "stage14-4-test"))]
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SocketEndpointV1 {
    pub family: u8,
    pub _reserved: u8,
    pub port: u16,
    pub address: [u8; 16],
}

#[cfg(any(feature = "stage14-3-test", feature = "stage14-4-test"))]
impl SocketEndpointV1 {
    pub const FAMILY_IPV4: u8 = 4;
    pub const FAMILY_IPV6: u8 = 6;

    pub fn from_endpoint(endpoint: IpEndpoint) -> Self {
        let mut address = [0u8; 16];
        let family = match endpoint.addr {
            IpAddress::Ipv4(ip) => {
                address[..4].copy_from_slice(&ip.octets());
                Self::FAMILY_IPV4
            }
            IpAddress::Ipv6(ip) => {
                address.copy_from_slice(&ip.octets());
                Self::FAMILY_IPV6
            }
        };
        Self {
            family,
            _reserved: 0,
            port: endpoint.port,
            address,
        }
    }

    pub fn to_endpoint(self) -> Result<IpEndpoint, SocketError> {
        if self.port == 0 || self._reserved != 0 {
            return Err(SocketError::Address);
        }
        let addr = match self.family {
            Self::FAMILY_IPV4 if self.address[4..].iter().all(|byte| *byte == 0) => {
                IpAddress::Ipv4(Ipv4Address::new(
                    self.address[0],
                    self.address[1],
                    self.address[2],
                    self.address[3],
                ))
            }
            Self::FAMILY_IPV6 => {
                let ip = SmolIpv6Address::from_octets(self.address);
                if ip.is_unspecified() || ip.is_multicast() {
                    return Err(SocketError::Address);
                }
                IpAddress::Ipv6(ip)
            }
            _ => return Err(SocketError::Address),
        };
        Ok(IpEndpoint::new(addr, self.port))
    }
}

#[cfg(feature = "stage14-3-test")]
pub fn stage14_3_socket_api_self_test() -> bool {
    let valid = IpEndpoint::new(
        IpAddress::Ipv4(Ipv4Address::new(10, 0, 2, 2)),
        443,
    );
    let packed = u64::from(u32::from_be_bytes([10, 0, 2, 2])) | (443u64 << 32);
    let unspecified = IpEndpoint::new(IpAddress::Ipv4(Ipv4Address::new(0, 0, 0, 0)), 443);
    let multicast = IpEndpoint::new(IpAddress::Ipv4(Ipv4Address::new(224, 0, 0, 1)), 443);

    let ipv4_v1 = SocketEndpointV1::from_endpoint(valid);
    let ipv6 = IpEndpoint::new(
        IpAddress::Ipv6(SmolIpv6Address::from_octets([
            0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
        ])),
        443,
    );
    let ipv6_v1 = SocketEndpointV1::from_endpoint(ipv6);
    let mut malformed_ipv4 = ipv4_v1;
    malformed_ipv4.address[15] = 1;
    let mut multicast_ipv6 = ipv6_v1;
    multicast_ipv6.address[0] = 0xff;

    valid_socket_endpoint(valid)
        && endpoint_from_packed(packed) == Ok(valid)
        && endpoint_to_packed(valid) == packed
        && !valid_socket_endpoint(unspecified)
        && !valid_socket_endpoint(multicast)
        && endpoint_from_packed(packed & 0xffff_ffff).is_err()
        && ipv4_v1.to_endpoint() == Ok(valid)
        && ipv6_v1.to_endpoint() == Ok(ipv6)
        && malformed_ipv4.to_endpoint() == Err(SocketError::Address)
        && multicast_ipv6.to_endpoint() == Err(SocketError::Address)
        && valid_socket_endpoint(ipv6)
}


#[cfg(feature = "stage14-3-test")]
pub fn stage14_3_socket_authority_self_test() -> bool {
    const OWNER_A: u64 = 0x143A;
    const OWNER_B: u64 = 0x143B;

    let Ok(id) = socket_open(OWNER_A, SocketKind::Udp) else {
        return false;
    };
    if socket_peer_v1(OWNER_B, id) != Err(SocketError::WrongOwner) {
        let _ = socket_close(OWNER_A, id);
        return false;
    }

    let Ok(token) = pin_socket(OWNER_A, id) else {
        let _ = socket_close(OWNER_A, id);
        return false;
    };
    if socket_close(OWNER_A, id).is_err()
        || socket_peer_v1(OWNER_A, id) != Err(SocketError::Invalid)
    {
        unpin_socket(token);
        return false;
    }

    // Closing a descriptor revokes descriptor authority immediately. The
    // pinned generation remains valid only until the outstanding async
    // reference is released; after slot reuse the old token must be rejected.
    unpin_socket(token);
    let Ok(reused) = socket_open(OWNER_A, SocketKind::Udp) else {
        return false;
    };
    if reused != id || socket_send_pinned(token, b"stale") != Err(SocketError::Invalid) {
        let _ = socket_close(OWNER_A, reused);
        return false;
    }

    let result = socket_peer_v1(OWNER_B, reused) == Err(SocketError::WrongOwner);
    let _ = socket_close(OWNER_A, reused);
    result && stats().user_sockets == 0
}

#[cfg(feature = "stage14-4-test")]
const MAX_WOVEN_ROUTES: usize = 8;

#[cfg(feature = "stage14-4-test")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteTableError {
    Capacity,
    InvalidPrefix,
    InvalidGateway,
    InvalidHandle,
}

#[cfg(feature = "stage14-4-test")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WovenRoute {
    pub network: Ipv4Address,
    pub prefix_len: u8,
    pub gateway: Ipv4Address,
    pub metric: u16,
    pub generation: u32,
}

#[cfg(feature = "stage14-4-test")]
#[derive(Clone, Copy)]
struct RouteSlot {
    route: Option<WovenRoute>,
}

#[cfg(feature = "stage14-4-test")]
pub struct WovenRouteTable {
    slots: [RouteSlot; MAX_WOVEN_ROUTES],
    next_generation: u32,
}

#[cfg(feature = "stage14-4-test")]
impl WovenRouteTable {
    pub const fn new() -> Self {
        Self {
            slots: [RouteSlot { route: None }; MAX_WOVEN_ROUTES],
            next_generation: 1,
        }
    }

    pub fn add(
        &mut self,
        network: Ipv4Address,
        prefix_len: u8,
        gateway: Ipv4Address,
        metric: u16,
    ) -> Result<WovenRoute, RouteTableError> {
        if prefix_len > 32 {
            return Err(RouteTableError::InvalidPrefix);
        }
        if gateway.octets() == [0; 4] {
            return Err(RouteTableError::InvalidGateway);
        }
        let index = self
            .slots
            .iter()
            .position(|slot| slot.route.is_none())
            .ok_or(RouteTableError::Capacity)?;
        let network_value = u32::from_be_bytes(network.octets()) & prefix_mask(prefix_len);
        let route = WovenRoute {
            network: Ipv4Address::from_octets(network_value.to_be_bytes()),
            prefix_len,
            gateway,
            metric,
            generation: self.next_generation,
        };
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        self.slots[index].route = Some(route);
        Ok(route)
    }

    pub fn remove(&mut self, route: WovenRoute) -> Result<(), RouteTableError> {
        let Some(slot) = self.slots.iter_mut().find(|slot| slot.route == Some(route)) else {
            return Err(RouteTableError::InvalidHandle);
        };
        slot.route = None;
        Ok(())
    }

    pub fn lookup(&self, destination: Ipv4Address) -> Option<WovenRoute> {
        self.slots
            .iter()
            .filter_map(|slot| slot.route)
            .filter(|route| ipv4_matches(route.network, destination, route.prefix_len))
            .min_by_key(|route| (u8::MAX - route.prefix_len, route.metric, route.generation))
    }
}

#[cfg(feature = "stage14-4-test")]
fn prefix_mask(prefix_len: u8) -> u32 {
    if prefix_len == 0 { 0 } else { u32::MAX << (32 - prefix_len) }
}

#[cfg(feature = "stage14-4-test")]
fn ipv4_matches(network: Ipv4Address, destination: Ipv4Address, prefix_len: u8) -> bool {
    let mask = prefix_mask(prefix_len);
    (u32::from_be_bytes(network.octets()) & mask)
        == (u32::from_be_bytes(destination.octets()) & mask)
}

#[cfg(feature = "stage14-4-test")]
pub fn stage14_4_routing_table_self_test() -> bool {
    let mut table = WovenRouteTable::new();
    let default = table.add(Ipv4Address::new(0, 0, 0, 0), 0, DEFAULT_GATEWAY, 100).ok();
    let broad = table.add(Ipv4Address::new(10, 0, 0, 0), 16, DEFAULT_GATEWAY, 100).ok();
    let specific = table.add(Ipv4Address::new(10, 0, 2, 0), 24, DEFAULT_GATEWAY, 50).ok();
    let Some(default) = default else { return false; };
    let Some(broad) = broad else { return false; };
    let Some(specific) = specific else { return false; };
    table.lookup(Ipv4Address::new(10, 0, 2, 15)) == Some(specific)
        && table.lookup(Ipv4Address::new(10, 0, 9, 15)) == Some(broad)
        && table.lookup(Ipv4Address::new(192, 0, 2, 1)) == Some(default)
        && table.remove(specific).is_ok()
        && table.lookup(Ipv4Address::new(10, 0, 2, 15)) == Some(broad)
        && table.remove(specific) == Err(RouteTableError::InvalidHandle)
}

pub fn endpoint_to_packed(endpoint: IpEndpoint) -> u64 {
    match endpoint.addr {
        IpAddress::Ipv4(ip) => {
            u64::from(u32::from_be_bytes(ip.octets())) | ((endpoint.port as u64) << 32)
        }
        IpAddress::Ipv6(_) => 0,
    }
}

pub fn stats() -> NetStats {
    let Some(runtime) = RUNTIME.get() else {
        return NetStats::default();
    };
    let runtime = runtime.lock();
    NetStats {
        online: virtio_net::is_initialized(),
        echo_active: runtime.echo_handle.is_some(),
        echo_port: runtime.echo_port,
        echo_packets: runtime.echo_packets,
        user_sockets: runtime.user.iter().filter(|s| s.is_some()).count(),
        dhcp_enabled: runtime.dhcp_enabled,
        using_dhcp: runtime.using_dhcp,
    }
}

pub fn initialized() -> bool {
    RUNTIME.get().is_some() && virtio_net::is_initialized()
}

pub fn self_test() -> bool {
    default_cidr().prefix_len() == DEFAULT_PREFIX
        && DEFAULT_GATEWAY == Ipv4Address::new(10, 0, 2, 2)
        && DEFAULT_DNS == Ipv4Address::new(10, 0, 2, 3)
        && virtio_net::self_test()
}

#[cfg(feature = "stage14-1-test")]
pub fn stage14_1_dhcp_transition_self_test() -> bool {
    let Some(runtime) = RUNTIME.get() else {
        return false;
    };
    let mut runtime = runtime.lock();

    let handle = match runtime.dns_handle {
        Some(handle) => handle,
        None => return false,
    };
    let query = {
        let Runtime { iface, sockets, .. } = &mut *runtime;
        match sockets.get_mut::<dns::Socket>(handle).start_query(
            iface.context(),
            "stage14-1-transition.invalid",
            smoltcp::wire::DnsQueryType::A,
        ) {
            Ok(query) => query,
            Err(_) => return false,
        }
    };
    let generation = runtime.next_dns_generation.max(1);
    runtime.next_dns_generation = generation.wrapping_add(1).max(1);
    runtime.dns_queries[0] = Some(DnsQuerySlot {
        handle: query,
        generation,
    });

    let lease_address = smoltcp::wire::Ipv4Cidr::new(Ipv4Address::new(10, 0, 2, 42), 24);
    let lease_router = Ipv4Address::new(10, 0, 2, 1);
    let lease_dns = Ipv4Address::new(10, 0, 2, 53);
    apply_dhcp_locked(&mut runtime, lease_address, lease_router, lease_dns);

    if !runtime.using_dhcp
        || runtime.ipv4 != lease_address.address()
        || runtime.prefix != lease_address.prefix_len()
        || runtime.gateway != lease_router
        || runtime.dns_server != lease_dns
        || runtime.dns_queries.iter().any(Option::is_some)
    {
        apply_static_locked(&mut runtime);
        return false;
    }

    apply_static_locked(&mut runtime);
    !runtime.using_dhcp
        && runtime.ipv4 == DEFAULT_IPV4
        && runtime.prefix == DEFAULT_PREFIX
        && runtime.gateway == DEFAULT_GATEWAY
        && runtime.dns_server == DEFAULT_DNS
        && runtime.dns_queries.iter().all(Option::is_none)
}

#[cfg(feature = "stage14-1-test")]
pub fn stage14_1_socket_stress_self_test() -> bool {
    const OWNER: u64 = 0x0001_401C;
    let fail = |checkpoint: &str| {
        crate::serial::write_line(format_args!(
            "[S14.1S-DIAG] FAILED checkpoint={checkpoint}"
        ));
        false
    };
    let mut ids = [0u64; MAX_USER_SOCKETS];

    for id in &mut ids {
        match socket_open(OWNER, SocketKind::Udp) {
            Ok(opened) => *id = opened,
            Err(_) => {
                close_process_sockets(OWNER);
                return fail("S1-open-exhaustion");
            }
        }
    }
    if socket_open(OWNER, SocketKind::Udp) != Err(SocketError::NoSlot) {
        close_process_sockets(OWNER);
        return fail("S2-noslot-boundary");
    }

    for id in ids {
        if socket_close(OWNER, id).is_err() {
            close_process_sockets(OWNER);
            return fail("S3-close");
        }
    }
    if stats().user_sockets != 0 {
        close_process_sockets(OWNER);
        return fail("S4-first-retirement");
    }

    let mut reopened = [u64::MAX; MAX_USER_SOCKETS];
    for index in 0..MAX_USER_SOCKETS {
        let Ok(opened) = socket_open(OWNER, SocketKind::Udp) else {
            close_process_sockets(OWNER);
            return fail("S5-reopen-capacity");
        };
        if opened as usize >= MAX_USER_SOCKETS || reopened[..index].contains(&opened) {
            close_process_sockets(OWNER);
            return fail("S6-reopen-identity");
        }
        reopened[index] = opened;
    }
    if socket_open(OWNER, SocketKind::Udp) != Err(SocketError::NoSlot) {
        close_process_sockets(OWNER);
        return fail("S7-reopen-noslot");
    }
    close_process_sockets(OWNER);
    if stats().user_sockets != 0 {
        return fail("S8-second-retirement");
    }

    let Some(runtime) = RUNTIME.get() else {
        return fail("S9-runtime");
    };
    let mut runtime = runtime.lock();
    let saved = runtime.next_ephemeral;
    runtime.next_ephemeral = 65533;
    let first = next_ephemeral(&mut runtime);
    let second = next_ephemeral(&mut runtime);
    let wrapped = next_ephemeral(&mut runtime);
    let after_wrap = next_ephemeral(&mut runtime);
    runtime.next_ephemeral = saved;

    if first != 65533 || second != 65534 || wrapped != 49152 || after_wrap != 49153 {
        return fail("S10-ephemeral-wrap");
    }
    true
}

#[cfg(feature = "stage14-1-test")]
pub fn stage14_1_lifecycle_self_test() -> bool {
    const OWNER_A: u64 = 0x0001_401A;
    const OWNER_B: u64 = 0x0001_401B;

    // Bounded DNS slots must be explicitly reclaimable even when a caller
    // abandons a pending lookup.
    let Ok(query) = dns_start("stage14-1.invalid") else {
        return false;
    };
    if dns_cancel(query).is_err() || dns_poll(query) != Err(SocketError::Invalid) {
        return false;
    }
    let Ok(reused_query) = dns_start("stage14-1-reuse.invalid") else {
        return false;
    };
    if reused_query == query
        || dns_poll(query) != Err(SocketError::Invalid)
        || dns_cancel(query) != Err(SocketError::Invalid)
    {
        let _ = dns_cancel(reused_query);
        return false;
    }
    if dns_cancel(reused_query).is_err() {
        return false;
    }

    // Descriptor ownership is immediate, while pinned async authority is tied
    // to the slot generation so a later occupant cannot inherit stale access.
    let Ok(id) = socket_open(OWNER_A, SocketKind::Udp) else {
        return false;
    };
    if socket_peer(OWNER_B, id) != Err(SocketError::WrongOwner) {
        let _ = socket_close(OWNER_A, id);
        return false;
    }
    let Ok(token) = pin_socket(OWNER_A, id) else {
        let _ = socket_close(OWNER_A, id);
        return false;
    };
    if socket_close(OWNER_A, id).is_err() {
        unpin_socket(token);
        return false;
    }
    if socket_peer(OWNER_A, id) != Err(SocketError::Invalid) {
        unpin_socket(token);
        return false;
    }
    unpin_socket(token);

    let Ok(reused_id) = socket_open(OWNER_A, SocketKind::Udp) else {
        return false;
    };
    if reused_id != id {
        let _ = socket_close(OWNER_A, reused_id);
        return false;
    }
    let stale_rejected = socket_send_pinned(token, b"x") == Err(SocketError::Invalid);
    let _ = socket_close(OWNER_A, reused_id);
    stale_rejected
}

fn now() -> Instant {
    let millis = timer::ticks().saturating_mul(1000) / timer::FREQUENCY_HZ as u64;
    Instant::from_millis(millis as i64)
}

/// Live QEMU/slirp network regression used by the Stage-5 release gate.
///
/// The Python harness supplies host forwards for UDP/7000 and TCP/8080 and
/// waits for the READY markers below before injecting traffic.  Keeping the
/// test in the kernel means DHCP, DNS, ICMP, UDP and TCP are exercised through
/// the same virtio-net/smoltcp runtime used by normal boots rather than through
/// a host-side mock.
#[cfg(any(feature = "qemu-test", feature = "network-test"))]
pub fn qemu_runtime_self_test() -> bool {
    const UDP_PORT: u16 = 7000;
    const TCP_PORT: u16 = 8080;
    const OWNER: u64 = u64::MAX - 0x5748;
    let dhcp_timeout = u64::from(timer::FREQUENCY_HZ) * 8;
    let dns_timeout = u64::from(timer::FREQUENCY_HZ) * 8;
    let ping_timeout = u64::from(timer::FREQUENCY_HZ) * 8;
    let io_timeout = u64::from(timer::FREQUENCY_HZ) * 12;

    if set_dhcp(true).is_err() {
        return false;
    }
    let deadline = timer::ticks().saturating_add(dhcp_timeout);
    while !stats().using_dhcp {
        poll();
        if timer::ticks() >= deadline {
            crate::serial::write_line(format_args!("[NETTEST] DHCP: TIMEOUT"));
            return false;
        }
        crate::task::yield_now();
    }
    crate::serial::write_line(format_args!("[NETTEST] DHCP: PASSED"));

    let Ok(query) = dns_start("example.com") else {
        crate::serial::write_line(format_args!("[NETTEST] DNS: START FAILED"));
        return false;
    };
    let deadline = timer::ticks().saturating_add(dns_timeout);
    loop {
        poll();
        match dns_poll(query) {
            Ok(Some(_)) => break,
            Ok(None) => {}
            Err(_) => {
                crate::serial::write_line(format_args!("[NETTEST] DNS: FAILED"));
                return false;
            }
        }
        if timer::ticks() >= deadline {
            crate::serial::write_line(format_args!("[NETTEST] DNS: TIMEOUT"));
            return false;
        }
        crate::task::yield_now();
    }
    crate::serial::write_line(format_args!("[NETTEST] DNS: PASSED"));

    if ping_start(DEFAULT_GATEWAY).is_err() {
        crate::serial::write_line(format_args!("[NETTEST] ICMP: START FAILED"));
        return false;
    }
    let deadline = timer::ticks().saturating_add(ping_timeout);
    loop {
        poll();
        match ping_poll() {
            Ok(Some(_)) => break,
            Ok(None) => {}
            Err(_) => {
                crate::serial::write_line(format_args!("[NETTEST] ICMP: FAILED"));
                return false;
            }
        }
        if timer::ticks() >= deadline {
            crate::serial::write_line(format_args!("[NETTEST] ICMP: TIMEOUT"));
            return false;
        }
        crate::task::yield_now();
    }
    crate::serial::write_line(format_args!("[NETTEST] ICMP: PASSED"));

    if start_udp_echo(UDP_PORT).is_err() {
        crate::serial::write_line(format_args!("[NETTEST] UDP: BIND FAILED"));
        return false;
    }
    let initial_packets = stats().echo_packets;
    crate::serial::write_line(format_args!("[NETTEST] UDP READY port={}", UDP_PORT));
    let deadline = timer::ticks().saturating_add(io_timeout);
    while stats().echo_packets == initial_packets {
        poll();
        if timer::ticks() >= deadline {
            crate::serial::write_line(format_args!("[NETTEST] UDP: TIMEOUT"));
            return false;
        }
        crate::task::yield_now();
    }
    crate::serial::write_line(format_args!("[NETTEST] UDP: PASSED"));

    let Ok(tcp_id) = socket_open(OWNER, SocketKind::Tcp) else {
        crate::serial::write_line(format_args!("[NETTEST] TCP: OPEN FAILED"));
        return false;
    };
    if socket_bind(OWNER, tcp_id, TCP_PORT).is_err() {
        let _ = socket_close(OWNER, tcp_id);
        crate::serial::write_line(format_args!("[NETTEST] TCP: LISTEN FAILED"));
        return false;
    }
    crate::serial::write_line(format_args!("[NETTEST] TCP READY port={}", TCP_PORT));

    let mut payload = [0u8; 256];
    let deadline = timer::ticks().saturating_add(io_timeout);
    let received = loop {
        poll();
        match socket_recv(OWNER, tcp_id, &mut payload) {
            Ok((len, _)) if len != 0 => break len,
            Ok(_) | Err(SocketError::WouldBlock) => {}
            Err(_) => {
                let _ = socket_close(OWNER, tcp_id);
                crate::serial::write_line(format_args!("[NETTEST] TCP: RECEIVE FAILED"));
                return false;
            }
        }
        if timer::ticks() >= deadline {
            let _ = socket_close(OWNER, tcp_id);
            crate::serial::write_line(format_args!("[NETTEST] TCP: RECEIVE TIMEOUT"));
            return false;
        }
        crate::task::yield_now();
    };

    let deadline = timer::ticks().saturating_add(io_timeout);
    loop {
        poll();
        match socket_send(OWNER, tcp_id, &payload[..received]) {
            Ok(len) if len == received => break,
            Ok(_) | Err(SocketError::WouldBlock) | Err(SocketError::BufferFull) => {}
            Err(_) => {
                let _ = socket_close(OWNER, tcp_id);
                crate::serial::write_line(format_args!("[NETTEST] TCP: SEND FAILED"));
                return false;
            }
        }
        if timer::ticks() >= deadline {
            let _ = socket_close(OWNER, tcp_id);
            crate::serial::write_line(format_args!("[NETTEST] TCP: SEND TIMEOUT"));
            return false;
        }
        crate::task::yield_now();
    }
    // Give smoltcp time to put the echoed bytes on the wire before removing the
    // socket.  The host harness verifies the bytes, so this is not just a
    // queueing test.
    for _ in 0..4 {
        poll();
        crate::task::yield_now();
    }
    let _ = socket_close(OWNER, tcp_id);
    close_process_sockets(OWNER);
    // The live acceptance owner must not leak a bounded userspace descriptor
    // into the Stage 14.1 resource-stress tests that run immediately after it.
    // TCP may need polling to complete its graceful close; bound that cleanup
    // by the same I/O window used by the live round-trip.
    let cleanup_deadline = timer::ticks().saturating_add(io_timeout);
    while stats().user_sockets != 0 {
        poll();
        if timer::ticks() >= cleanup_deadline {
            crate::serial::write_line(format_args!("[NETTEST] TCP: CLEANUP TIMEOUT"));
            return false;
        }
        crate::task::yield_now();
    }
    crate::serial::write_line(format_args!("[NETTEST] TCP: PASSED"));
    true
}
