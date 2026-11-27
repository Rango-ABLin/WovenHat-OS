//! Bounded SOCKS5 CONNECT transport for native network workers.
//!
//! Target names (including .onion) travel in ATYP=DOMAIN requests. This module
//! never performs DNS or direct target connections. A configured SOCKS endpoint
//! is not proof of Tor identity, a bootstrapped circuit, TLS, or a working VPN.
use crate::net::{self, Endpoint, TcpStream};
pub const GREETING: [u8; 3] = [5, 1, 0];
pub const MAX_PACKET: usize = 262;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidHost,
    InvalidPort,
    OutputTooSmall,
    InvalidReply,
    AuthenticationRequired,
    ProxyRejected(u8),
    Transport(net::Error),
}
impl From<net::Error> for Error {
    fn from(error: net::Error) -> Self {
        Self::Transport(error)
    }
}

pub fn encode_connect(host: &str, port: u16, output: &mut [u8]) -> Result<usize, Error> {
    if host.is_empty()
        || host.len() > 253
        || !host.is_ascii()
        || host.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || !label.as_bytes()[0].is_ascii_alphanumeric()
                || !label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
                || !label
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        })
    {
        return Err(Error::InvalidHost);
    }
    if port == 0 {
        return Err(Error::InvalidPort);
    }
    let length = 7 + host.len();
    if output.len() < length {
        return Err(Error::OutputTooSmall);
    }
    output[..5].copy_from_slice(&[5, 1, 0, 3, host.len() as u8]);
    output[5..5 + host.len()].copy_from_slice(host.as_bytes());
    output[5 + host.len()..length].copy_from_slice(&port.to_be_bytes());
    Ok(length)
}

pub fn check_method(reply: [u8; 2]) -> Result<(), Error> {
    if reply[0] != 5 {
        return Err(Error::InvalidReply);
    }
    if reply[1] != 0 {
        return Err(Error::AuthenticationRequired);
    }
    Ok(())
}

/// Returns the CONNECT reply's frame length, leaving any following stream bytes
/// to the caller. Incomplete frames return None, malformed frames are refused.
pub fn reply_length(reply: &[u8]) -> Result<Option<usize>, Error> {
    if reply.len() < 4 {
        return Ok(None);
    }
    if reply[0] != 5 || reply[2] != 0 {
        return Err(Error::InvalidReply);
    }
    if reply[1] != 0 {
        return Err(Error::ProxyRejected(reply[1]));
    }
    let length = match reply[3] {
        1 => 10,
        4 => 22,
        3 => {
            let Some(&length) = reply.get(4) else {
                return Ok(None);
            };
            if length == 0 {
                return Err(Error::InvalidReply);
            }
            7 + usize::from(length)
        }
        _ => return Err(Error::InvalidReply),
    };
    Ok((reply.len() >= length).then_some(length))
}

/// Connect only to the explicit proxy, then request remote name resolution.
/// Runs on a network worker; each operation uses an explicit deadline in 100 Hz kernel ticks.
/// Failure drops the socket; no direct target or local DNS fallback exists.
pub fn connect(proxy: Endpoint, host: &str, port: u16, timeout: u64) -> Result<TcpStream, Error> {
    let mut packet = [0u8; MAX_PACKET];
    // Validate before allocating a socket or touching the network.
    let length = encode_connect(host, port, &mut packet)?;
    net::validate_timeout(timeout)?;
    proxy.packed()?;
    let socket = TcpStream::new()?;
    socket.connect(proxy)?.wait_timeout(&mut [], timeout)?;
    socket.write_all_timeout(&GREETING, timeout)?;
    let mut method = [0; 2];
    socket.read_exact_timeout(&mut method, timeout)?;
    check_method(method)?;
    socket.write_all_timeout(&packet[..length], timeout)?;
    socket.read_exact_timeout(&mut packet[..4], timeout)?;
    // Validate fixed fields before trusting the variable address length.
    reply_length(&packet[..4])?;
    let length = match packet[3] {
        1 => 10,
        4 => 22,
        3 => {
            socket.read_exact_timeout(&mut packet[4..5], timeout)?;
            if packet[4] == 0 {
                return Err(Error::InvalidReply);
            }
            7 + usize::from(packet[4])
        }
        _ => return Err(Error::InvalidReply),
    };
    let filled = if packet[3] == 3 { 5 } else { 4 };
    socket.read_exact_timeout(&mut packet[filled..length], timeout)?;
    if reply_length(&packet[..length])? != Some(length) {
        return Err(Error::InvalidReply);
    }
    Ok(socket)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_request_is_refused_before_native_socket_creation() {
        let proxy = Endpoint {
            address: [127, 0, 0, 1],
            port: 9050,
        };
        assert!(matches!(
            connect(proxy, "", 80, 200),
            Err(Error::InvalidHost)
        ));
        assert!(matches!(
            connect(proxy, "example.com", 80, u64::MAX),
            Err(Error::Transport(net::Error::InvalidTimeout))
        ));
        let invalid = Endpoint {
            address: [0; 4],
            port: 0,
        };
        assert!(matches!(
            connect(invalid, "example.com", 80, 200),
            Err(Error::Transport(net::Error::InvalidEndpoint))
        ));
    }
    #[test]
    fn names_and_onion_targets_use_remote_resolution() {
        let mut packet = [0; MAX_PACKET];
        let n = encode_connect("example.onion", 80, &mut packet).unwrap();
        assert_eq!(&packet[..5], &[5, 1, 0, 3, 13]);
        assert_eq!(&packet[5..n - 2], b"example.onion");
        assert_eq!(&packet[n - 2..n], &[0, 80]);
    }
    #[test]
    fn malformed_names_and_capacities_leave_output_unchanged() {
        for host in [
            "",
            ".onion",
            "a..onion",
            "-a.onion",
            "a-.onion",
            "user@host",
            "host/path",
            "host:80",
            "h\r\nost",
            "h ost",
            "h\\ost",
            "é.com",
        ] {
            let mut bytes = [7; MAX_PACKET];
            assert_eq!(
                encode_connect(host, 80, &mut bytes),
                Err(Error::InvalidHost)
            );
            assert_eq!(bytes, [7; MAX_PACKET]);
        }
        assert_eq!(
            encode_connect("host", 0, &mut [0; 20]),
            Err(Error::InvalidPort)
        );
        assert_eq!(
            encode_connect("host", 80, &mut [0; 10]),
            Err(Error::OutputTooSmall)
        );
        assert_eq!(
            encode_connect(&"a".repeat(64), 80, &mut [0; MAX_PACKET]),
            Err(Error::InvalidHost)
        );
    }
    #[test]
    fn fragmented_and_coalesced_reply_boundaries() {
        let ipv4 = [5, 0, 0, 1, 127, 0, 0, 1, 0, 80];
        for length in 0..ipv4.len() {
            assert_eq!(reply_length(&ipv4[..length]), Ok(None));
        }
        assert_eq!(reply_length(&ipv4), Ok(Some(10)));
        let mut ipv6 = [0; 22];
        ipv6[..4].copy_from_slice(&[5, 0, 0, 4]);
        assert_eq!(reply_length(&ipv6), Ok(Some(22)));
        assert_eq!(
            reply_length(&[5, 0, 0, 3, 1, b'a', 0, 80, b'H']),
            Ok(Some(8))
        );
        assert_eq!(reply_length(&[5, 0, 0, 3, 0]), Err(Error::InvalidReply));
    }
    #[test]
    fn authentication_and_proxy_refusal_do_not_succeed() {
        assert_eq!(check_method([5, 0]), Ok(()));
        assert_eq!(check_method([4, 0]), Err(Error::InvalidReply));
        assert_eq!(check_method([5, 2]), Err(Error::AuthenticationRequired));
        for status in 1..=255 {
            assert_eq!(
                reply_length(&[5, status, 0, 1]),
                Err(Error::ProxyRejected(status))
            );
        }
        for reply in [[4, 0, 0, 1], [5, 0, 1, 1], [5, 0, 0, 9]] {
            assert_eq!(reply_length(&reply), Err(Error::InvalidReply));
        }
    }
}
