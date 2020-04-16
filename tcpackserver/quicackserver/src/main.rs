use argh::FromArgs;
use tokio::pin;
use tokio::select;
use tokio::stream;
use tokio::task;
use futures_util::{SinkExt, StreamExt};
use futures_codec::{FramedRead, FramedWrite, LinesCodec};

use std::error::Error;
use std::time::{Instant, Duration};
use std::io;
use quinn::{ServerConfig, PrivateKey, TransportConfig, ServerConfigBuilder, Certificate, CertificateChain, ClientConfig, ClientConfigBuilder, Endpoint};
use std::sync::Arc;

#[derive(FromArgs)]
/// Reach new heights.
struct Config {
    /// size of payload
    #[argh(option, short = 'p', default = "1024")]
    payload_size: usize,

    /// number of messages
    #[argh(option, short = 'n', default = "10000")]
    count: usize,
}

/// Returns default server configuration along with its certificate.
fn configure_server() -> Result<(ServerConfig, Vec<u8>), Box<dyn Error>> {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let cert_der = cert.serialize_der().unwrap();
    let priv_key = cert.serialize_private_key_der();
    let priv_key = PrivateKey::from_der(&priv_key)?;

    let mut transport_config = TransportConfig::default();
    transport_config.stream_window_uni(0);
    let mut server_config = ServerConfig::default();
    server_config.transport = Arc::new(transport_config);
    let mut cfg_builder = ServerConfigBuilder::new(server_config);
    let cert = Certificate::from_der(&cert_der)?;
    cfg_builder.certificate(CertificateChain::from_certs(vec![cert]), priv_key)?;

    Ok((cfg_builder.build(), cert_der))
}

fn configure_client(server_certs: &[&[u8]]) -> Result<ClientConfig, Box<dyn Error>> {
    let mut cfg_builder = ClientConfigBuilder::default();
    for cert in server_certs {
        cfg_builder.add_certificate_authority(Certificate::from_der(&cert)?)?;
    }
    Ok(cfg_builder.build())
}

async fn server(server_config: ServerConfig) -> Result<(), io::Error> {
    let mut endpoint_builder = Endpoint::builder();
    endpoint_builder.listen(server_config);

    let bind_addr = "127.0.0.1:8080".parse().unwrap();
    let (_endpoint, mut incoming) = endpoint_builder.bind(&bind_addr).unwrap();

    loop {
        let conn = incoming.next().await.unwrap();
        let new_connection = conn.await.unwrap();
        println!("[server] connection accepted: addr={}", new_connection.connection.remote_address());
        let quinn::NewConnection { connection, .. } = new_connection;
        task::spawn(async move {
            let (tx, rx) = connection.open_bi().await.unwrap();
            let mut tx = FramedWrite::new(tx, LinesCodec);
            let mut rx = FramedRead::new(rx, LinesCodec);

            while let Some(line) = rx.next().await {
                println!("-> {:?}", line);
                tx.send("ack\n".to_owned()).await.unwrap();
            }
        });
    }
}

async fn client(payload_size: usize, max_count: usize, server_certs: &[&[u8]]) -> Result<(), io::Error> {
    let client_cfg = configure_client(server_certs).unwrap();
    let mut endpoint_builder = Endpoint::builder();
    endpoint_builder.default_client_config(client_cfg);
    let bind_addr = "0.0.0.0:0".parse().unwrap();
    let (endpoint, _incoming) = endpoint_builder.bind(&bind_addr).unwrap();

    let server_addr = "127.0.0.1:8080".parse().unwrap();
    let connect = endpoint.connect(&server_addr, "localhost").unwrap();
    let quinn::NewConnection { connection, .. } = connect.await.unwrap();
    println!("[client] connected: addr={}", connection.remote_address());

    let (tx, rx) = connection.open_bi().await.unwrap();
    let mut tx = FramedWrite::new(tx, LinesCodec);
    let mut rx = FramedRead::new(rx, LinesCodec);

    let stream: Vec<usize> = (0..max_count).collect();
    let stream = stream::iter(stream);

    pin!(stream);
    let mut count = 0;
    let payload = common::generate_string(payload_size) + "\n";

    loop {
        select! {
            Some(_) = stream.next() => {
                tx.send(payload.clone()).await.unwrap();
            }
            Some(_data) = rx.next() => {
                 count += 1;
                 if count >= max_count {
                    break;
                 }
            }
        }
    }

    Ok(())
}

#[tokio::main(core_threads = 2)]
async fn main() -> Result<(), Box<dyn Error>> {
    let config: Config = argh::from_env();
    let count = config.count;
    let payload_size = config.payload_size;

    let (server_config, server_cert) = configure_server().unwrap();
    task::spawn(async move {
        server(server_config).await.unwrap();
    });

    tokio::time::delay_for(Duration::from_millis(1)).await;
    let start = Instant::now();
    client(payload_size, count, &[&server_cert]).await.unwrap();

    let elapsed = start.elapsed();
    let throughput = (config.payload_size * config.count as usize) as u128 / elapsed.as_millis();
    let throughput_secs = throughput * 1000;
    let throughput_secs_mb = throughput_secs / 1024 / 1024;
    println!("throughput = {} MB/s", throughput_secs_mb);
    Ok(())
}
