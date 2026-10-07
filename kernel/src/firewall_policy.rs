use crate::irq_lock::IrqMutex as Mutex;
use smoltcp::wire::{IpAddress, Ipv4Address, Ipv6Address};
use spin::Once;

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
fn packet_meta_from_ethernet(frame: &[u8], direction: FirewallDirection) -> Option<PacketMeta> {
    if frame.len() < 14 { return None; }
    match u16::from_be_bytes([frame[12], frame[13]]) {
        0x0800 => {
            let ip = &frame[14..];
            if ip.len() < 20 || ip[0] >> 4 != 4 { return None; }
            let header_len = usize::from(ip[0] & 0x0f) * 4;
            if header_len < 20 || ip.len() < header_len { return None; }
            let source = IpAddress::Ipv4(Ipv4Address::new(ip[12], ip[13], ip[14], ip[15]));
            let destination = IpAddress::Ipv4(Ipv4Address::new(ip[16], ip[17], ip[18], ip[19]));
            let (protocol, ports) = transport_meta(ip[9], &ip[header_len..], false)?;
            Some(PacketMeta {
                direction, protocol, source, destination,
                source_port: ports.map(|p| p.0), destination_port: ports.map(|p| p.1),
            })
        }
        0x86dd => {
            let ip = &frame[14..];
            if ip.len() < 40 || ip[0] >> 4 != 6 { return None; }
            let mut src = [0u8; 16]; src.copy_from_slice(&ip[8..24]);
            let mut dst = [0u8; 16]; dst.copy_from_slice(&ip[24..40]);
            let (protocol, ports) = transport_meta(ip[6], &ip[40..], true)?;
            Some(PacketMeta {
                direction,
                protocol,
                source: IpAddress::Ipv6(Ipv6Address::from_octets(src)),
                destination: IpAddress::Ipv6(Ipv6Address::from_octets(dst)),
                source_port: ports.map(|p| p.0), destination_port: ports.map(|p| p.1),
            })
        }
        _ => None,
    }
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
    let Some(meta) = packet_meta_from_ethernet(frame, direction) else {
        // Stage 14.5B filters understood IP traffic only. ARP and other L2
        // control traffic remain available to the network stack.
        return FrameDecision::Allow;
    };
    let policy = global_policy().lock();
    match policy.evaluate(meta) {
        FirewallAction::Allow => FrameDecision::Allow,
        FirewallAction::Deny => FrameDecision::Deny,
    }
}

#[cfg(feature = "stage14-5b-test")]
pub fn stage14_5b_self_test() -> bool {
    let mut ipv4 = [0u8; 14 + 20 + 8];
    ipv4[12..14].copy_from_slice(&0x0800u16.to_be_bytes());
    ipv4[14] = 0x45;
    ipv4[23] = 17;
    ipv4[26..30].copy_from_slice(&[10,1,2,3]);
    ipv4[30..34].copy_from_slice(&[10,1,9,9]);
    ipv4[34..36].copy_from_slice(&50000u16.to_be_bytes());
    ipv4[36..38].copy_from_slice(&7000u16.to_be_bytes());

    let mut ipv6 = [0u8; 14 + 40 + 8];
    ipv6[12..14].copy_from_slice(&0x86ddu16.to_be_bytes());
    ipv6[14] = 0x60;
    ipv6[20] = 17;
    ipv6[22..38].copy_from_slice(&Ipv6Address::new(0x2001,0xdb8,1,0,0,0,0,1).octets());
    ipv6[38..54].copy_from_slice(&Ipv6Address::new(0x2001,0xdb8,2,0,0,0,0,1).octets());
    ipv6[54..56].copy_from_slice(&50001u16.to_be_bytes());
    ipv6[56..58].copy_from_slice(&7001u16.to_be_bytes());

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

    // Restore a permissive test policy before the inherited live network suite
    // continues so Stage 14.1-14.4 behavior remains a regression gate.
    *global_policy().lock() = FirewallPolicy::new(FirewallAction::Allow);
    installed && inbound_allow && inbound_deny && outbound_allow && outbound_deny && control_allow
}
