#[cfg(feature = "console")]
mod tui;

use clap::Parser;
use std::net::{Ipv4Addr, SocketAddr};
use std::str::FromStr;
use std::time::Duration;
use vaux_broker::config::{DEFAULT_LISTEN_ADDR, DEFAULT_PORT, DEFAULT_TLS_PORT};
use vaux_broker::{Broker, Config};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about=None)]
struct Args {
    #[clap(short = 'l', long)]
    listen_addr: Option<String>,
    #[clap(short, long)]
    port: Option<u16>,
    #[clap(
        short = 's',
        long,
        help = "maximum number of sessions including inactive"
    )]
    max_sessions: Option<u32>,
    #[clap(short = 'a', long, help = "maximum number of active sessions")]
    max_active_sessions: Option<u32>,
    #[clap(short = 'x', long, help = "session expiration in seconds")]
    session_expiration: Option<u32>,
    #[clap(short = 't', long, help = "default session timeout in seconds")]
    session_timeout: Option<u32>,
    #[clap(short = 'T', long, help = "maximum allowed session timeout in seconds")]
    max_session_timeout: Option<u32>,
    #[clap(short = 'U', long, help = "run terminal user interface")]
    tui: bool,
    #[clap(long, help = "path to server certificate PEM file")]
    tls_cert: Option<String>,
    #[clap(long, help = "path to server private key PEM file")]
    tls_key: Option<String>,
}

fn load_tls_acceptor(
    cert_path: &str,
    key_path: &str,
) -> Result<tokio_rustls::TlsAcceptor, Box<dyn std::error::Error>> {
    use rustls::pki_types::PrivateKeyDer;
    use std::io::BufReader;

    let cert_file = std::fs::File::open(cert_path)
        .map_err(|e| format!("unable to open cert file '{cert_path}': {e}"))?;
    let key_file = std::fs::File::open(key_path)
        .map_err(|e| format!("unable to open key file '{key_path}': {e}"))?;

    let certs: Vec<_> = rustls_pemfile::certs(&mut BufReader::new(cert_file))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("unable to parse certs: {e}"))?;

    if certs.is_empty() {
        return Err("no certificates found in cert file".into());
    }

    let key: PrivateKeyDer = rustls_pemfile::private_key(&mut BufReader::new(key_file))
        .map_err(|e| format!("unable to parse private key: {e}"))?
        .ok_or("no private key found in key file")?;

    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| format!("invalid cert/key: {e}"))?;

    Ok(tokio_rustls::TlsAcceptor::from(std::sync::Arc::new(config)))
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let broker_version = env!("CARGO_PKG_VERSION");
    println!("{:-<1$}", "", 40);
    println!("-{:^38}-", format!("vaux MQTT broker v{}", broker_version));
    println!("{:-<1$}", "", 40);
    println!("\nCTRL-C to exit\n");

    let listen_addr = if let Some(listen_addr) = &args.listen_addr {
        if let Ok(addr) = Ipv4Addr::from_str(listen_addr) {
            addr
        } else {
            panic!("Listen address, \"{listen_addr}\" is not a valid IPV4 address");
        }
    } else {
        Ipv4Addr::from_str(DEFAULT_LISTEN_ADDR).unwrap()
    };

    let tls_acceptor = match (&args.tls_cert, &args.tls_key) {
        (Some(cert), Some(key)) => {
            Some(load_tls_acceptor(cert, key).unwrap_or_else(|e| {
                panic!("TLS configuration error: {e}");
            }))
        }
        (Some(_), None) => panic!("--tls-cert requires --tls-key"),
        (None, Some(_)) => panic!("--tls-key requires --tls-cert"),
        (None, None) => None,
    };

    let default_port = if tls_acceptor.is_some() {
        DEFAULT_TLS_PORT
    } else {
        DEFAULT_PORT
    };
    let listen_port = args.port.unwrap_or(default_port);
    let listen_addr = SocketAddr::from((listen_addr, listen_port));

    let mut config = Config::new(listen_addr);
    config.tls_acceptor = tls_acceptor;
    if let Some(expiration) = args.session_expiration {
        config.session_expiry = Duration::from_secs(expiration as u64);
    }

    let tls_status = if config.tls_acceptor.is_some() {
        "TLS enabled"
    } else {
        "plaintext"
    };

    let mut broker = Broker::new_with_config(config);
    let result = broker.run().await;
    println!("Broker started on: {listen_addr} ({tls_status})");
    if let Err(e) = result {
        eprintln!("Error starting broker: {e}");
    }

    tokio::signal::ctrl_c()
        .await
        .expect("failed to listen for ctrl-c");
    println!("\nShutting down...");
    broker.stop().await;
}
