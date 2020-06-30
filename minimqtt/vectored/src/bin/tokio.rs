use argh::FromArgs;

use std::error::Error;
use std::time::{Instant, Duration};

use futures_util::stream;
use tokio::io::AsyncWriteExt;
use tokio::stream::StreamExt;
use std::io;
use tokio::task;
use tokio::select;
use vectored::{mqtt_write, Publish, Packet, PubAck};
use vectored::codec::tokio::MqttCodec;
use tokio_util::codec::Framed;
use tokio::net::{TcpStream, TcpListener};
use bytes::buf::Buf;
use bytes::BytesMut;

#[derive(FromArgs)]
/// Reach new heights.
struct Config {
    /// size of payload
    #[argh(option, short = 'p', default = "16")]
    payload_size: usize,
    /// number of messages
    #[argh(option, short = 'n', default = "10_000_000")]
    count: usize,
    /// number of messages
    #[argh(option, short = 'f', default = "100")]
    flow_control_size: usize,
}

async fn server() -> Result<(), io::Error> {
    let mut listener = TcpListener::bind("127.0.0.1:8080").await?;

    loop {
        let (socket, _) = listener.accept().await?;
        task::spawn(async move {
            let mut out = BytesMut::new();
            let mut frames = Framed::new(socket, MqttCodec);
            while let Some(packet) = frames.next().await {
                match packet.unwrap() {
                    Packet::Publish(publish) => {
                        let ack = Packet::PubAck(PubAck {pkid: publish.pkid });
                        mqtt_write(ack, &mut out);
                        frames.get_mut().write_buf(&mut out).await.unwrap();
                    }
                    Packet::PubAck(_puback) =>  {}
                };
            }
        }).await.unwrap();
    }
}

async fn client(config: Config) -> Result<(), io::Error> {
    let socket = TcpStream::connect("127.0.0.1:8080").await.unwrap();
    let mut frames = Framed::new(socket, MqttCodec);
    let mut stream = stream::iter(packets(config.payload_size, config.count));

    let mut acked = 0;
    let mut sent = 0;
    let start = Instant::now();
    let mut out = BytesMut::new();
    loop {
        select! {
            // sent - acked guard prevents bounded queue deadlock ( assuming 100 packets doesn't
            // cause framed.send() to block )
            Some(packet) = stream.next(), if sent - acked < config.flow_control_size => {
                let mut payload = mqtt_write(packet, &mut out);
                sent += 1;
                frames.get_mut().write_buf(&mut out).await.unwrap();
                if let Some(p) = &mut payload {
                    frames.get_mut().write_buf(p).await.unwrap();
                }
            }
            Some(o) = frames.next() => match o.unwrap() {
                Packet::Publish(_publish) => (),
                Packet::PubAck(_ack) => {
                    acked += 1;
                    if acked >= config.count {
                        break;
                    }
                },
            },
            else => {
                println!("All branches disabled");
                break
            }
        }
    }

    let elapsed = start.elapsed();
    let throughput = (acked as usize) as u128 / elapsed.as_millis();
    let throughput_secs = throughput * 1000;
    println!("Id = tokio, Total = {}, Payload size (bytes) = {}, Flow control window len = {}, Throughput (messages/sec) = {}", acked, config.payload_size, config.flow_control_size, throughput_secs);
    Ok(())
}

#[tokio::main(core_threads = 2)]
async fn main() -> Result<(), Box<dyn Error>> {
    let config: Config = argh::from_env();

    let _server = task::spawn(server());
    tokio::time::delay_for(Duration::from_millis(1)).await;
    client(config).await.unwrap();
    Ok(())
}


pub fn packets(size: usize, count: usize) -> Vec<Packet> {
    let mut out = Vec::new();
    for i in 0..count {
        let pkid = (i % 65000) as u16 + 1 ;
        let payload = vec![i as u8; size];
        let packet = Publish::new(pkid, "hello/mqtt/topic/bytes", payload);
        out.push(Packet::Publish(packet))
    }

    out
}
