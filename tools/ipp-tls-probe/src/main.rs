use std::env;
use std::fs;
use std::path::Path;
use std::time::Duration;

use libwoven::ipp::{Client, Request};
use libwoven::ipp_tls::RustlsTransport;
use libwoven::web::parse_endpoint;
use rustls::pki_types::CertificateDer;
use rustls::RootCertStore;

fn fail(message: impl AsRef<str>) -> ! {
    eprintln!("{}", message.as_ref());
    std::process::exit(2);
}

fn main() {
    let mut args = env::args().skip(1);
    let uri = args.next().unwrap_or_else(|| fail("usage: ipp-tls-probe URI NAME [CA_DER]"));
    let name = args.next().unwrap_or_else(|| fail("usage: ipp-tls-probe URI NAME [CA_DER]"));
    let ca_der = args.next();
    if args.next().is_some() {
        fail("usage: ipp-tls-probe URI NAME [CA_DER]");
    }
    let endpoint = parse_endpoint(&uri).unwrap_or_else(|_| fail("invalid IPP URI"));
    if !endpoint.secure {
        fail("IPP probe requires an https:// URI");
    }
    let mut roots = RootCertStore::empty();
    if let Some(path) = ca_der {
        let bytes = fs::read(Path::new(&path)).unwrap_or_else(|error| fail(format!("read CA DER: {error}")));
        roots
            .add(CertificateDer::from(bytes))
            .unwrap_or_else(|error| fail(format!("invalid CA DER: {error}")));
    } else {
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    }
    let transport = RustlsTransport::with_roots(roots, Duration::from_secs(10))
        .unwrap_or_else(|_| fail("TLS transport configuration rejected"));
    let uri_static: &'static str = Box::leak(uri.clone().into_boxed_str());
    let name_static: &'static str = Box::leak(name.into_boxed_str());
    let mut client = Client::connect(transport, uri_static, name_static)
        .unwrap_or_else(|error| fail(format!("IPP client rejected endpoint: {error:?}")));
    let mut request = Request::new(0x000B, 1);
    request
        .add_text(0x01, "attributes-charset", "utf-8")
        .and_then(|_| request.add_text(0x01, "attributes-natural-language", "en"))
        .and_then(|_| request.add_text(0x01, "printer-uri", &uri))
        .unwrap_or_else(|error| fail(format!("request construction failed: {error:?}")));
    let mut response = [0u8; 4096];
    let result = client
        .submit(&request, &mut response)
        .unwrap_or_else(|error| fail(format!("secure IPP request failed: {error:?}")));
    println!("TLS IPP probe passed: status=0x{:04x} request_id={}", result.status, result.request_id);
}
