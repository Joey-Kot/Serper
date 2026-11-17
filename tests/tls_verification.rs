//! Verify HTTPS rejects an untrusted server and does not use CA overrides.

use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::{ServerConfig, ServerConnection, StreamOwned};
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Command, Output};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const SERVER_CERT: &[u8] = include_bytes!("fixtures/test-server-cert.der");
const SERVER_KEY: &[u8] = include_bytes!("fixtures/test-server-key.der");
const TEST_CA: &[u8] = include_bytes!("fixtures/test-ca.pem");

fn accept_with_timeout(listener: &TcpListener) -> TcpStream {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match listener.accept() {
            Ok((stream, _)) => return stream,
            Err(err) if err.kind() == ErrorKind::WouldBlock && Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(err) => panic!("accepting test proxy connection: {err}"),
        }
    }
}

fn serve_untrusted_proxy(listener: TcpListener) -> Vec<bool> {
    let cert = CertificateDer::from(SERVER_CERT.to_vec());
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(SERVER_KEY.to_vec()));
    let config = Arc::new(
        ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert], key)
            .unwrap(),
    );
    let mut rejected = Vec::new();
    for _ in 0..3 {
        let mut tcp = accept_with_timeout(&listener);
        tcp.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        tcp.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut reader = BufReader::new(tcp.try_clone().unwrap());
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        assert!(line.starts_with("CONNECT google.serper.dev:443 "), "{line}");
        loop {
            line.clear();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
            assert!(!line.is_empty(), "incomplete CONNECT request");
        }
        tcp.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .unwrap();
        let connection = ServerConnection::new(config.clone()).unwrap();
        let mut tls = StreamOwned::new(connection, tcp);
        let mut request = String::new();
        let result = BufReader::new(&mut tls).read_line(&mut request);
        rejected.push(!matches!(result, Ok(n) if n > 0));
    }
    rejected
}

fn run_credit(proxy: &str, env_var: Option<(&str, &Path)>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_serper"));
    command
        .args(["credit", "--timeout", "5"])
        .env("SERPER_KEY", "test-key")
        .env("HTTPS_PROXY", proxy)
        .env("https_proxy", proxy)
        .env("NO_PROXY", "")
        .env("no_proxy", "")
        .env_remove("SSL_CERT_FILE")
        .env_remove("SSL_CERT_DIR");
    if let Some((name, path)) = env_var {
        command.env(name, path);
    }
    command.output().unwrap()
}

#[test]
fn untrusted_server_is_rejected_even_if_ca_overrides_are_set() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let proxy = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || serve_untrusted_proxy(listener));
    let directory = tempfile::tempdir().unwrap();
    let ca_file = directory.path().join("test-ca.pem");
    std::fs::write(&ca_file, TEST_CA).unwrap();

    let results = [
        run_credit(&proxy, None),
        run_credit(&proxy, Some(("SSL_CERT_FILE", &ca_file))),
        run_credit(&proxy, Some(("SSL_CERT_DIR", directory.path()))),
    ];
    let rejected = server.join().unwrap();
    for result in results {
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8_lossy(&result.stderr).contains("account request error"));
    }
    assert_eq!(rejected, [true, true, true]);
}
