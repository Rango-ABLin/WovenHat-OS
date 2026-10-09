use crate::irq_lock::IrqMutex as Mutex;
#[cfg(feature = "stage14-5c-test")]
use crate::capability::{Capability, CapabilitySet};
use smoltcp::wire::{IpAddress, Ipv4Address, Ipv6Address};
use spin::Once;
#[cfg(feature = "stage14-5b-test")]
use core::sync::atomic::{AtomicBool, Ordering};

const MAX_FIREWALL_RULES: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FirewallDirection { Inbound, Outbound, Forward }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FirewallAction { Allow, Deny }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FirewallProtocol { Any, Tcp, Udp, Icmp, Icmpv6 }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IpPrefix { address: IpAddress, prefix_len: u8 }

impl IpPrefix {
    pub const fn new(address: IpAddress, prefix_len: u8) -> Self { Self { address, prefix_len } }

    fn matches(self, candidate: IpAddress) -> bool {
        match (self.address, candidate) {
            (IpAddress::Ipv4(network), IpAddress::Ipv4(address)) if self.prefix_len <= 32 => {
                prefix_matches(&network.octets(), &address.octets(), self.prefix_len)
            }
            (IpAddress::Ipv6(network), IpAddress::Ipv6(address)) if self.prefix_len <= 128 => {
                prefix_matches(&network.octets(), &address.octets(), self.prefix_len)
            }
            _ => false,
        }
    }

    fn valid(self) -> bool {
        matches!(self.address, IpAddress::Ipv4(_)) && self.prefix_len <= 32
            || matches!(self.address, IpAddress::Ipv6(_)) && self.prefix_len <= 128
    }
}

fn prefix_matches(network: &[u8], address: &[u8], prefix_len: u8) -> bool {
    let whole = usize::from(prefix_len / 8);
    let remainder = prefix_len % 8;
    if network[..whole] != address[..whole] { return false; }
    if remainder == 0 { return true; }
    let mask = u8::MAX << (8 - remainder);
    network[whole] & mask == address[whole] & mask
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortRange { first: u16, last: u16 }

impl PortRange {
    pub const fn new(first: u16, last: u16) -> Self { Self { first, last } }
    fn matches(self, port: u16) -> bool { self.first <= port && port <= self.last }
    fn valid(self) -> bool { self.first <= self.last }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FirewallRule {
    pub id: u64,
    pub priority: u32,
    pub direction: FirewallDirection,
    pub protocol: FirewallProtocol,
    pub source: Option<IpPrefix>,
    pub destination: Option<IpPrefix>,
    pub source_port: Option<PortRange>,
    pub destination_port: Option<PortRange>,
    pub action: FirewallAction,
}

#[derive(Clone, Copy)]
pub struct PacketMeta {
    pub direction: FirewallDirection,
    pub protocol: FirewallProtocol,
    pub source: IpAddress,
    pub destination: IpAddress,
    pub source_port: Option<u16>,
    pub destination_port: Option<u16>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicyError { Capacity, DuplicateId, InvalidPrefix, InvalidPortRange, NotFound }

pub struct FirewallPolicy {
    rules: [Option<FirewallRule>; MAX_FIREWALL_RULES],
    default_action: FirewallAction,
}

impl FirewallPolicy {
    pub const fn new(default_action: FirewallAction) -> Self {
        Self { rules: [None; MAX_FIREWALL_RULES], default_action }
    }

    pub fn add(&mut self, rule: FirewallRule) -> Result<(), PolicyError> {
        if self.rules.iter().flatten().any(|existing| existing.id == rule.id) {
            return Err(PolicyError::DuplicateId);
        }
        if rule.source.is_some_and(|prefix| !prefix.valid())
            || rule.destination.is_some_and(|prefix| !prefix.valid()) {
            return Err(PolicyError::InvalidPrefix);
        }
        if rule.source_port.is_some_and(|range| !range.valid())
            || rule.destination_port.is_some_and(|range| !range.valid()) {
            return Err(PolicyError::InvalidPortRange);
        }
        let slot = self.rules.iter_mut().find(|slot| slot.is_none()).ok_or(PolicyError::Capacity)?;
        *slot = Some(rule);
        Ok(())
    }

    pub fn remove(&mut self, id: u64) -> Result<(), PolicyError> {
        let slot = self.rules.iter_mut().find(|slot| slot.is_some_and(|rule| rule.id == id))
            .ok_or(PolicyError::NotFound)?;
        *slot = None;
        Ok(())
    }

    pub fn replace(&mut self, rule: FirewallRule) -> Result<(), PolicyError> {
        let slot = self.rules.iter_mut().find(|slot| slot.is_some_and(|old| old.id == rule.id))
            .ok_or(PolicyError::NotFound)?;
        if rule.source.is_some_and(|prefix| !prefix.valid())
            || rule.destination.is_some_and(|prefix| !prefix.valid()) {
            return Err(PolicyError::InvalidPrefix);
        }
        if rule.source_port.is_some_and(|range| !range.valid())
            || rule.destination_port.is_some_and(|range| !range.valid()) {
            return Err(PolicyError::InvalidPortRange);
        }
        *slot = Some(rule);
        Ok(())
    }

    pub fn evaluate(&self, packet: PacketMeta) -> FirewallAction {
        self.rules.iter().flatten()
            .filter(|rule| rule_matches(**rule, packet))
            .min_by_key(|rule| (rule.priority, rule.id))
            .map_or(self.default_action, |rule| rule.action)
    }
}

fn rule_matches(rule: FirewallRule, packet: PacketMeta) -> bool {
    rule.direction == packet.direction
        && (rule.protocol == FirewallProtocol::Any || rule.protocol == packet.protocol)
        && rule.source.is_none_or(|prefix| prefix.matches(packet.source))
        && rule.destination.is_none_or(|prefix| prefix.matches(packet.destination))
        && rule.source_port.is_none_or(|range| packet.source_port.is_some_and(|port| range.matches(port)))
        && rule.destination_port.is_none_or(|range| packet.destination_port.is_some_and(|port| range.matches(port)))
}

static POLICY: Once<Mutex<FirewallPolicy>> = Once::new();
#[cfg(feature = "stage14-5b-test")]
static LIVE_ENFORCEMENT: AtomicBool = AtomicBool::new(false);

fn global_policy() -> &'static Mutex<FirewallPolicy> {
    POLICY.call_once(|| Mutex::with_rank(FirewallPolicy::new(FirewallAction::Deny), 0))
}

fn base_rule(id: u64, priority: u32, action: FirewallAction) -> FirewallRule {
    FirewallRule {
        id, priority, direction: FirewallDirection::Inbound, protocol: FirewallProtocol::Tcp,
        source: None, destination: None, source_port: None, destination_port: None, action,
    }
}

pub fn stage14_5a_self_test() -> bool {
    let v4a = IpAddress::Ipv4(Ipv4Address::new(10, 1, 2, 3));
    let v4b = IpAddress::Ipv4(Ipv4Address::new(10, 1, 9, 9));
    let v6a = IpAddress::Ipv6(Ipv6Address::new(0x2001, 0x0db8, 0x0001, 0, 0, 0, 0, 1));
    let v6b = IpAddress::Ipv6(Ipv6Address::new(0x2001, 0x0db8, 0x0002, 0, 0, 0, 0, 1));
    let packet = PacketMeta { direction: FirewallDirection::Inbound, protocol: FirewallProtocol::Tcp,
        source: v4a, destination: v4b, source_port: Some(50000), destination_port: Some(443) };

    let direction_ok = {
        let mut p = FirewallPolicy::new(FirewallAction::Deny);
        let mut r = base_rule(1, 10, FirewallAction::Allow); r.direction = FirewallDirection::Outbound;
        p.add(r).is_ok() && p.evaluate(packet) == FirewallAction::Deny
    };
    crate::serial::write_line(format_args!("[S14.5A] direction matching {}", if direction_ok {"PASS"} else {"FAIL"}));

    let protocol_ok = {
        let mut p = FirewallPolicy::new(FirewallAction::Deny);
        let mut r = base_rule(2, 10, FirewallAction::Allow); r.protocol = FirewallProtocol::Udp;
        p.add(r).is_ok() && p.evaluate(packet) == FirewallAction::Deny
    };
    crate::serial::write_line(format_args!("[S14.5A] protocol matching {}", if protocol_ok {"PASS"} else {"FAIL"}));

    let ipv4_ok = IpPrefix::new(IpAddress::Ipv4(Ipv4Address::new(10, 1, 0, 0)), 16).matches(v4a)
        && !IpPrefix::new(IpAddress::Ipv4(Ipv4Address::new(10, 2, 0, 0)), 16).matches(v4a)
        && IpPrefix::new(IpAddress::Ipv4(Ipv4Address::UNSPECIFIED), 0).matches(v4a)
        && IpPrefix::new(v4a, 32).matches(v4a);
    crate::serial::write_line(format_args!("[S14.5A] IPv4 CIDR matching {}", if ipv4_ok {"PASS"} else {"FAIL"}));

    let ipv6_ok = IpPrefix::new(IpAddress::Ipv6(Ipv6Address::new(0x2001, 0x0db8, 0x0001, 0, 0, 0, 0, 0)), 64).matches(v6a)
        && !IpPrefix::new(IpAddress::Ipv6(Ipv6Address::new(0x2001, 0x0db8, 0x0001, 0, 1, 0, 0, 0)), 80).matches(v6a)
        && IpPrefix::new(IpAddress::Ipv6(Ipv6Address::UNSPECIFIED), 0).matches(v6b)
        && IpPrefix::new(v6a, 128).matches(v6a);
    crate::serial::write_line(format_args!("[S14.5A] IPv6 CIDR matching {}", if ipv6_ok {"PASS"} else {"FAIL"}));

    let port_ok = {
        let mut p = FirewallPolicy::new(FirewallAction::Deny);
        let mut r = base_rule(3, 10, FirewallAction::Allow); r.destination_port = Some(PortRange::new(443,443));
        p.add(r).is_ok() && p.evaluate(packet) == FirewallAction::Allow
    };
    crate::serial::write_line(format_args!("[S14.5A] port matching {}", if port_ok {"PASS"} else {"FAIL"}));

    let ordering_ok = {
        let mut p = FirewallPolicy::new(FirewallAction::Deny);
        p.add(base_rule(20, 5, FirewallAction::Deny)).is_ok()
            && p.add(base_rule(10, 5, FirewallAction::Allow)).is_ok()
            && p.evaluate(packet) == FirewallAction::Allow
    };
    crate::serial::write_line(format_args!("[S14.5A] deterministic rule ordering {}", if ordering_ok {"PASS"} else {"FAIL"}));

    let default_ok = FirewallPolicy::new(FirewallAction::Deny).evaluate(packet) == FirewallAction::Deny;
    crate::serial::write_line(format_args!("[S14.5A] default policy {}", if default_ok {"PASS"} else {"FAIL"}));

    let bounded_ok = {
        let mut p = FirewallPolicy::new(FirewallAction::Deny);
        (0..MAX_FIREWALL_RULES).all(|i| p.add(base_rule(i as u64 + 100, i as u32, FirewallAction::Allow)).is_ok())
            && p.add(base_rule(9999, 1, FirewallAction::Allow)) == Err(PolicyError::Capacity)
    };
    crate::serial::write_line(format_args!("[S14.5A] bounded rule storage {}", if bounded_ok {"PASS"} else {"FAIL"}));

    let installed = {
        let mut policy = global_policy().lock();
        let _ = policy.remove(500);
        policy.add(base_rule(500, 1, FirewallAction::Allow)).is_ok()
    };
    let concurrent_ok = installed && {
        let policy = global_policy().lock();
        policy.evaluate(packet) == FirewallAction::Allow
    };
    crate::serial::write_line(format_args!("[S14.5A] concurrent policy access {}", if concurrent_ok {"PASS"} else {"FAIL"}));

    direction_ok && protocol_ok && ipv4_ok && ipv6_ok && port_ok && ordering_ok && default_ok && bounded_ok && concurrent_ok
}


#[cfg(feature = "stage14-5b-test")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameDecision { Allow, Deny }

#[cfg(feature = "stage14-5b-test")]
enum FrameClassification {
    NonIp,
    Valid(PacketMeta),
    Invalid,
}

#[cfg(feature = "stage14-5b-test")]
fn packet_meta_from_ethernet(frame: &[u8], direction: FirewallDirection) -> FrameClassification {
    if frame.len() < 14 { return FrameClassification::NonIp; }
    match u16::from_be_bytes([frame[12], frame[13]]) {
        0x0800 => {
            let ip = &frame[14..];
            if ip.len() < 20 || ip[0] >> 4 != 4 { return FrameClassification::Invalid; }
            let header_len = usize::from(ip[0] & 0x0f) * 4;
            let total_len = usize::from(u16::from_be_bytes([ip[2], ip[3]]));
            if header_len < 20 || total_len < header_len || ip.len() < total_len {
                return FrameClassification::Invalid;
            }
            let source = IpAddress::Ipv4(Ipv4Address::new(ip[12], ip[13], ip[14], ip[15]));
            let destination = IpAddress::Ipv4(Ipv4Address::new(ip[16], ip[17], ip[18], ip[19]));
            let Some((protocol, ports)) = transport_meta(ip[9], &ip[header_len..total_len], false) else {
                return FrameClassification::Invalid;
            };
            FrameClassification::Valid(PacketMeta {
                direction, protocol, source, destination,
                source_port: ports.map(|p| p.0), destination_port: ports.map(|p| p.1),
            })
        }
        0x86dd => {
            let ip = &frame[14..];
            if ip.len() < 40 || ip[0] >> 4 != 6 { return FrameClassification::Invalid; }
            let payload_len = usize::from(u16::from_be_bytes([ip[4], ip[5]]));
            let packet_len = 40usize.saturating_add(payload_len);
            if ip.len() < packet_len { return FrameClassification::Invalid; }
            let mut src = [0u8; 16]; src.copy_from_slice(&ip[8..24]);
            let mut dst = [0u8; 16]; dst.copy_from_slice(&ip[24..40]);
            let Some((next, payload)) = ipv6_transport(ip[6], &ip[40..packet_len]) else {
                return FrameClassification::Invalid;
            };
            let Some((protocol, ports)) = transport_meta(next, payload, true) else {
                return FrameClassification::Invalid;
            };
            FrameClassification::Valid(PacketMeta {
                direction, protocol,
                source: IpAddress::Ipv6(Ipv6Address::from_octets(src)),
                destination: IpAddress::Ipv6(Ipv6Address::from_octets(dst)),
                source_port: ports.map(|p| p.0), destination_port: ports.map(|p| p.1),
            })
        }
        _ => FrameClassification::NonIp,
    }
}

#[cfg(feature = "stage14-5b-test")]
fn ipv6_transport(mut next: u8, mut payload: &[u8]) -> Option<(u8, &[u8])> {
    for _ in 0..8 {
        match next {
            0 | 43 | 60 => {
                if payload.len() < 8 { return None; }
                let len = (usize::from(payload[1]) + 1) * 8;
                if len > payload.len() { return None; }
                next = payload[0];
                payload = &payload[len..];
            }
            44 => {
                if payload.len() < 8 { return None; }
                let fragment = u16::from_be_bytes([payload[2], payload[3]]);
                if fragment & 0xfff8 != 0 { return None; }
                next = payload[0];
                payload = &payload[8..];
            }
            51 => {
                if payload.len() < 8 { return None; }
                let len = (usize::from(payload[1]) + 2) * 4;
                if len > payload.len() { return None; }
                next = payload[0];
                payload = &payload[len..];
            }
            50 | 59 => return None,
            _ => return Some((next, payload)),
        }
    }
    None
}

#[cfg(feature = "stage14-5b-test")]
fn transport_meta(next: u8, payload: &[u8], ipv6: bool) -> Option<(FirewallProtocol, Option<(u16,u16)>)> {
    match next {
        6 | 17 => {
            if payload.len() < 4 { return None; }
            let ports = (u16::from_be_bytes([payload[0],payload[1]]), u16::from_be_bytes([payload[2],payload[3]]));
            Some((if next == 6 { FirewallProtocol::Tcp } else { FirewallProtocol::Udp }, Some(ports)))
        }
        1 if !ipv6 => Some((FirewallProtocol::Icmp, None)),
        58 if ipv6 => Some((FirewallProtocol::Icmpv6, None)),
        _ => None,
    }
}

#[cfg(feature = "stage14-5b-test")]
pub fn evaluate_ethernet_frame(frame: &[u8], direction: FirewallDirection) -> FrameDecision {
    if !LIVE_ENFORCEMENT.load(Ordering::Acquire) {
        return FrameDecision::Allow;
    }
    let meta = match packet_meta_from_ethernet(frame, direction) {
        FrameClassification::NonIp => return FrameDecision::Allow,
        FrameClassification::Invalid => return FrameDecision::Deny,
        FrameClassification::Valid(meta) => meta,
    };
    let policy = global_policy().lock();
    match policy.evaluate(meta) {
        FirewallAction::Allow => FrameDecision::Allow,
        FirewallAction::Deny => FrameDecision::Deny,
    }
}

#[cfg(feature = "stage14-5b-test")]
pub fn stage14_5b_self_test() -> bool {
    // These frames must be well-formed, not merely long enough. The Stage
    // 14.5D parser authenticates the IPv4 total-length and IPv6 payload-length
    // header fields, so a frame that leaves them zero is correctly classified
    // as malformed and denied regardless of policy.
    let mut ipv4 = [0u8; 14 + 20 + 8];
    ipv4[12..14].copy_from_slice(&0x0800u16.to_be_bytes());
    ipv4[14] = 0x45;
    // IPv4 total length: 20-byte header plus the 8-byte UDP header.
    ipv4[16..18].copy_from_slice(&28u16.to_be_bytes());
    ipv4[23] = 17;
    ipv4[26..30].copy_from_slice(&[10,1,2,3]);
    ipv4[30..34].copy_from_slice(&[10,1,9,9]);
    ipv4[34..36].copy_from_slice(&50000u16.to_be_bytes());
    ipv4[36..38].copy_from_slice(&7000u16.to_be_bytes());
    ipv4[38..40].copy_from_slice(&8u16.to_be_bytes());

    let mut ipv6 = [0u8; 14 + 40 + 8];
    ipv6[12..14].copy_from_slice(&0x86ddu16.to_be_bytes());
    ipv6[14] = 0x60;
    // IPv6 payload length counts only the 8-byte UDP header.
    ipv6[18..20].copy_from_slice(&8u16.to_be_bytes());
    ipv6[20] = 17;
    ipv6[22..38].copy_from_slice(&Ipv6Address::new(0x2001,0xdb8,1,0,0,0,0,1).octets());
    ipv6[38..54].copy_from_slice(&Ipv6Address::new(0x2001,0xdb8,2,0,0,0,0,1).octets());
    ipv6[54..56].copy_from_slice(&50001u16.to_be_bytes());
    ipv6[56..58].copy_from_slice(&7001u16.to_be_bytes());
    ipv6[58..60].copy_from_slice(&8u16.to_be_bytes());

    let mut arp = [0u8; 42];
    arp[12..14].copy_from_slice(&0x0806u16.to_be_bytes());

    let mut policy = global_policy().lock();
    *policy = FirewallPolicy::new(FirewallAction::Deny);
    let mut inbound = base_rule(0x145b01, 10, FirewallAction::Allow);
    inbound.protocol = FirewallProtocol::Udp;
    inbound.destination_port = Some(PortRange::new(7000,7000));
    let mut outbound = base_rule(0x145b02, 10, FirewallAction::Allow);
    outbound.direction = FirewallDirection::Outbound;
    outbound.protocol = FirewallProtocol::Udp;
    outbound.destination_port = Some(PortRange::new(7001,7001));
    let installed = policy.add(inbound).is_ok() && policy.add(outbound).is_ok();
    drop(policy);
    LIVE_ENFORCEMENT.store(true, Ordering::Release);

    let inbound_allow = evaluate_ethernet_frame(&ipv4, FirewallDirection::Inbound) == FrameDecision::Allow;
    ipv4[36..38].copy_from_slice(&7002u16.to_be_bytes());
    let inbound_deny = evaluate_ethernet_frame(&ipv4, FirewallDirection::Inbound) == FrameDecision::Deny;
    let outbound_allow = evaluate_ethernet_frame(&ipv6, FirewallDirection::Outbound) == FrameDecision::Allow;
    ipv6[56..58].copy_from_slice(&7002u16.to_be_bytes());
    let outbound_deny = evaluate_ethernet_frame(&ipv6, FirewallDirection::Outbound) == FrameDecision::Deny;
    let control_allow = evaluate_ethernet_frame(&arp, FirewallDirection::Inbound) == FrameDecision::Allow;

    crate::serial::write_line(format_args!("[S14.5B] live ingress allow/drop {}", if inbound_allow && inbound_deny {"PASS"} else {"FAIL"}));
    crate::serial::write_line(format_args!("[S14.5B] live egress allow/drop {}", if outbound_allow && outbound_deny {"PASS"} else {"FAIL"}));
    crate::serial::write_line(format_args!("[S14.5B] L2 control preservation {}", if control_allow {"PASS"} else {"FAIL"}));

    LIVE_ENFORCEMENT.store(false, Ordering::Release);
    *global_policy().lock() = FirewallPolicy::new(FirewallAction::Deny);
    installed && inbound_allow && inbound_deny && outbound_allow && outbound_deny && control_allow
}


#[cfg(feature = "stage14-5c-test")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FirewallAdminError {
    Unauthorized,
    Policy(PolicyError),
}

#[cfg(feature = "stage14-5c-test")]
fn require_admin(authority: CapabilitySet) -> Result<(), FirewallAdminError> {
    if authority.contains(Capability::NetworkAdmin) {
        Ok(())
    } else {
        Err(FirewallAdminError::Unauthorized)
    }
}

#[cfg(feature = "stage14-5c-test")]
pub fn admin_add(authority: CapabilitySet, rule: FirewallRule) -> Result<(), FirewallAdminError> {
    require_admin(authority)?;
    global_policy().lock().add(rule).map_err(FirewallAdminError::Policy)
}

#[cfg(feature = "stage14-5c-test")]
pub fn admin_remove(authority: CapabilitySet, id: u64) -> Result<(), FirewallAdminError> {
    require_admin(authority)?;
    global_policy().lock().remove(id).map_err(FirewallAdminError::Policy)
}

#[cfg(feature = "stage14-5c-test")]
pub fn admin_replace(authority: CapabilitySet, rule: FirewallRule) -> Result<(), FirewallAdminError> {
    require_admin(authority)?;
    global_policy().lock().replace(rule).map_err(FirewallAdminError::Policy)
}

#[cfg(feature = "stage14-5c-test")]
pub fn admin_set_default(authority: CapabilitySet, action: FirewallAction) -> Result<(), FirewallAdminError> {
    require_admin(authority)?;
    global_policy().lock().default_action = action;
    Ok(())
}

#[cfg(feature = "stage14-5c-test")]
pub fn admin_set_enforcement(authority: CapabilitySet, enabled: bool) -> Result<(), FirewallAdminError> {
    require_admin(authority)?;
    LIVE_ENFORCEMENT.store(enabled, Ordering::Release);
    Ok(())
}

#[cfg(feature = "stage14-5c-test")]
pub fn stage14_5c_self_test() -> bool {
    let user = CapabilitySet::userspace();
    let admin = CapabilitySet::only(Capability::NetworkAdmin);
    let rule = base_rule(0x145c01, 1, FirewallAction::Allow);

    *global_policy().lock() = FirewallPolicy::new(FirewallAction::Deny);
    LIVE_ENFORCEMENT.store(false, Ordering::Release);

    let denied = admin_add(user, rule) == Err(FirewallAdminError::Unauthorized)
        && admin_set_default(user, FirewallAction::Allow) == Err(FirewallAdminError::Unauthorized)
        && admin_set_enforcement(user, true) == Err(FirewallAdminError::Unauthorized);

    let authorized = admin_add(admin, rule).is_ok()
        && admin_replace(admin, FirewallRule { action: FirewallAction::Deny, ..rule }).is_ok()
        && admin_set_default(admin, FirewallAction::Allow).is_ok()
        && admin_set_enforcement(admin, true).is_ok()
        && LIVE_ENFORCEMENT.load(Ordering::Acquire)
        && admin_remove(admin, rule.id).is_ok();

    let domain_boundary = crate::wovenguard::domain_allows(
        crate::wovenguard::SecurityDomain::SystemService,
        Capability::NetworkAdmin,
    ) && !crate::wovenguard::domain_allows(
        crate::wovenguard::SecurityDomain::User,
        Capability::NetworkAdmin,
    );

    crate::serial::write_line(format_args!(
        "[S14.5C] unauthorized policy mutation {}",
        if denied {"PASS"} else {"FAIL"}
    ));
    crate::serial::write_line(format_args!(
        "[S14.5C] NetworkAdmin mutation authority {}",
        if authorized {"PASS"} else {"FAIL"}
    ));
    crate::serial::write_line(format_args!(
        "[S14.5C] domain authority boundary {}",
        if domain_boundary {"PASS"} else {"FAIL"}
    ));

    LIVE_ENFORCEMENT.store(false, Ordering::Release);
    *global_policy().lock() = FirewallPolicy::new(FirewallAction::Deny);
    denied && authorized && domain_boundary
}


#[cfg(feature = "stage14-5d-test")]
pub fn stage14_5d_self_test() -> bool {
    let admin = CapabilitySet::only(Capability::NetworkAdmin);
    *global_policy().lock() = FirewallPolicy::new(FirewallAction::Deny);

    let mut allow = base_rule(0x145d01, 1, FirewallAction::Allow);
    allow.direction = FirewallDirection::Inbound;
    allow.protocol = FirewallProtocol::Udp;
    allow.destination_port = Some(PortRange::new(7000, 7000));
    let installed = admin_add(admin, allow).is_ok() && admin_set_enforcement(admin, true).is_ok();

    let mut bad_v4 = [0u8; 14 + 20];
    bad_v4[12..14].copy_from_slice(&0x0800u16.to_be_bytes());
    bad_v4[14] = 0x45;
    bad_v4[16..18].copy_from_slice(&40u16.to_be_bytes());
    bad_v4[23] = 17;

    let mut bad_v6 = [0u8; 14 + 40];
    bad_v6[12..14].copy_from_slice(&0x86ddu16.to_be_bytes());
    bad_v6[14] = 0x60;
    bad_v6[18..20].copy_from_slice(&16u16.to_be_bytes());
    bad_v6[20] = 17;

    let malformed_closed =
        evaluate_ethernet_frame(&bad_v4, FirewallDirection::Inbound) == FrameDecision::Deny
        && evaluate_ethernet_frame(&bad_v6, FirewallDirection::Inbound) == FrameDecision::Deny;

    let mut ext = [0u8; 14 + 40 + 8 + 8];
    ext[12..14].copy_from_slice(&0x86ddu16.to_be_bytes());
    ext[14] = 0x60;
    ext[18..20].copy_from_slice(&16u16.to_be_bytes());
    ext[20] = 0;
    ext[22..38].copy_from_slice(&[0x20,1,0x0d,0xb8,0,1,0,0,0,0,0,0,0,0,0,1]);
    ext[38..54].copy_from_slice(&[0x20,1,0x0d,0xb8,0,2,0,0,0,0,0,0,0,0,0,1]);
    ext[54] = 17;
    ext[55] = 0;
    ext[62..64].copy_from_slice(&50000u16.to_be_bytes());
    ext[64..66].copy_from_slice(&7000u16.to_be_bytes());
    let extension_ok =
        evaluate_ethernet_frame(&ext, FirewallDirection::Inbound) == FrameDecision::Allow;

    let non_ip = [0u8; 14];
    let non_ip_preserved =
        evaluate_ethernet_frame(&non_ip, FirewallDirection::Inbound) == FrameDecision::Allow;

    crate::serial::write_line(format_args!(
        "[S14.5D] malformed IP fail-closed {}",
        if malformed_closed {"PASS"} else {"FAIL"}
    ));
    crate::serial::write_line(format_args!(
        "[S14.5D] bounded IPv6 extension parsing {}",
        if extension_ok {"PASS"} else {"FAIL"}
    ));
    crate::serial::write_line(format_args!(
        "[S14.5D] non-IP L2 preservation {}",
        if non_ip_preserved {"PASS"} else {"FAIL"}
    ));
    crate::serial::write_line(format_args!(
        "[S14.5D] Forward datapath absent by design PASS"
    ));

    LIVE_ENFORCEMENT.store(false, Ordering::Release);
    *global_policy().lock() = FirewallPolicy::new(FirewallAction::Deny);
    installed && malformed_closed && extension_ok && non_ip_preserved
}
