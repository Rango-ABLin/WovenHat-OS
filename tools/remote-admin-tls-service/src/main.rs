use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::Arc;

use libwoven::remote_admin::{decode_request, PolicyJournal, Request};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::server::WebPkiClientVerifier;
use rustls::{ServerConfig, ServerConnection, StreamOwned};
use sha2::{Digest, Sha256};

const HEADER: usize = 26;
const MAX_FRAME: usize = HEADER + 256;

struct AuthorizedClient;
impl libwoven::remote_admin::Authorizer for AuthorizedClient {
    fn authorize(&mut self, _request: &Request, _payload: &[u8]) -> bool {
        true
    }
}

fn fail(message: impl AsRef<str>) -> ! {
    eprintln!("{}", message.as_ref());
    std::process::exit(2);
}

fn read_der(path: &str) -> Vec<u8> {
    fs::read(Path::new(path)).unwrap_or_else(|error| fail(format!("read DER {path}: {error}")))
}

fn fingerprint(value: &str) -> [u8; 32] {
    let mut result = [0u8; 32];
    if value.len() != 64 {
        fail("client certificate fingerprint must be 64 hexadecimal digits");
    }
    for (index, byte) in result.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .unwrap_or_else(|_| fail("client certificate fingerprint is not hexadecimal"));
    }
    result
}

fn config(server_cert: &str, server_key: &str, client_ca: &str) -> Arc<ServerConfig> {
    let cert = CertificateDer::from(read_der(server_cert));
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(read_der(server_key)));
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(CertificateDer::from(read_der(client_ca)))
        .unwrap_or_else(|error| fail(format!("invalid client CA: {error}")));
    let verifier = WebPkiClientVerifier::builder(Arc::new(roots))
        .build()
        .unwrap_or_else(|error| fail(format!("client verifier configuration failed: {error}")));
    Arc::new(
        ServerConfig::builder()
            .with_client_cert_verifier(verifier)
            .with_single_cert(vec![cert], key)
            .unwrap_or_else(|error| fail(format!("server certificate configuration failed: {error}"))),
    )
}

fn serve(mut stream: StreamOwned<ServerConnection, TcpStream>, expected_client: [u8; 32]) {
    let peer = stream
        .conn
        .peer_certificates()
        .and_then(|certs| certs.first())
        .map(|cert| Sha256::digest(cert.as_ref()));
    if peer.as_ref().is_none_or(|digest| digest.as_slice() != expected_client) {
        fail("authenticated client certificate fingerprint mismatch");
    }
    let mut journal = PolicyJournal::new(b"safe").unwrap_or_else(|_| fail("initial policy invalid"));
    loop {
        let mut frame = [0u8; MAX_FRAME];
        stream
            .read_exact(&mut frame[..HEADER])
            .unwrap_or_else(|error| fail(format!("read WADM header: {error}")));
        let length = u16::from_be_bytes([frame[24], frame[25]]) as usize;
        if length > 256 {
            fail("WADM payload exceeds bound");
        }
        stream
            .read_exact(&mut frame[HEADER..HEADER + length])
            .unwrap_or_else(|error| fail(format!("read WADM payload: {error}")));
        let mut payload = [0u8; 256];
        let request = decode_request(&frame[..HEADER + length], &mut payload)
            .unwrap_or_else(|error| fail(format!("invalid WADM request: {error:?}")));
        let mut authorizer = AuthorizedClient;
        let result = match request.command {
            libwoven::remote_admin::Command::ApplyPolicy => journal.apply(
                &mut authorizer,
                request.nonce,
                request.expected_generation,
                &payload[..request.payload_len],
            ),
            libwoven::remote_admin::Command::RollbackPolicy => {
                journal.rollback(&mut authorizer, request.nonce, request.expected_generation)
            }
            libwoven::remote_admin::Command::QueryPolicy => Ok(journal.view()),
        };
        let (status, generation, length) = match result {
            Ok(view) => (0u8, view.generation, view.length),
            Err(_) => (1u8, journal.view().generation, journal.view().length),
        };
        let mut response = [0u8; 16];
        response[..4].copy_from_slice(b"WADR");
        response[4] = 1;
        response[5] = status;
        response[8..16].copy_from_slice(&generation.to_be_bytes());
        response[6..8].copy_from_slice(&(length as u16).to_be_bytes());
        stream
            .write_all(&response)
            .unwrap_or_else(|error| fail(format!("write WADR response: {error}")));
        stream.flush().unwrap_or_else(|error| fail(format!("flush WADR response: {error}")));
    }
}

fn main() {
    let mut args = env::args().skip(1);
    let address = args.next().unwrap_or_else(|| fail("usage: service LISTEN SERVER_CERT_DER SERVER_KEY_DER CLIENT_CA_DER CLIENT_CERT_SHA256"));
    let server_cert = args.next().unwrap_or_else(|| fail("missing server certificate"));
    let server_key = args.next().unwrap_or_else(|| fail("missing server private key"));
    let client_ca = args.next().unwrap_or_else(|| fail("missing client CA"));
    let client_fingerprint = args.next().unwrap_or_else(|| fail("missing client certificate fingerprint"));
    if args.next().is_some() {
        fail("too many arguments");
    }
    let listener = TcpListener::bind(&address).unwrap_or_else(|error| fail(format!("bind {address}: {error}")));
    let config = config(&server_cert, &server_key, &client_ca);
    println!("remote-admin TLS listener ready on {address}");
    let (stream, peer) = listener.accept().unwrap_or_else(|error| fail(format!("accept: {error}")));
    println!("accepted authenticated-admin connection from {peer}");
    let connection = ServerConnection::new(config).unwrap_or_else(|error| fail(format!("TLS setup: {error}")));
    serve(StreamOwned::new(connection, stream), fingerprint(&client_fingerprint));
}
