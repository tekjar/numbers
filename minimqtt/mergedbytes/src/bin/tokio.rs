use argh::FromArgs;

use std::error::Error;
use std::time::{Instant, Duration};

use futures_util::stream;
use futures_util::{SinkExt, StreamExt};
use std::io;
use tokio::task;
use tokio::select;
use mergedbytes::{Publish, Packet, PubAck};
use mergedbytes::codec::tokio::MqttCodec;
use tokio_util::codec::Framed;
use tokio::net::{TcpStream, TcpListener};

#[derive(FromArgs)]
/// Reach new heights.
struct Config {
    /// size of payload
    #[argh(option, short = 'p', default = "16")]
    payload_size: usize,
    /// number of messages
    #[argh(option, short = 'n', default = "10_000_000")]
    count: usize,
}

async fn server() -> Result<(), io::Error> {
    let mut listener = TcpListener::bind("127.0.0.1:8080").await?;

    loop {
        let (socket, _) = listener.accept().await?;
        task::spawn(async move {
            let mut frames = Framed::new(socket, MqttCodec);
            while let Some(packet) = frames.next().await {
                match packet.unwrap() {
                    Packet::Publish(publish) => {
                        let ack = Packet::PubAck(PubAck {pkid: publish.pkid });
                        frames.send(ack).await.unwrap();
                    }
                    Packet::PubAck(_puback) =>  {}
                };
            }
        }).await.unwrap();
    }
}

async fn client(payload_size: usize, max_count: usize) -> Result<(), io::Error> {
    let socket = TcpStream::connect("127.0.0.1:8080").await.unwrap();
    let mut frames = Framed::new(socket, MqttCodec);
    let mut stream = stream::iter(packets(payload_size, max_count));

    let mut acked = 0;
    let mut sent = 0;
    let start = Instant::now();
    loop {
        select! {
            // sent - acked guard prevents bounded queue deadlock ( assuming 100 packets doesn't
            // cause framed.send() to block )
            Some(packet) = stream.next(), if sent - acked < 100 => {
                frames.send(packet).await.unwrap();
                sent += 1;
            }
            Some(o) = frames.next() => match o.unwrap() {
                Packet::Publish(_publish) => (),
                Packet::PubAck(_ack) => {
                    acked += 1;
                    if acked >= max_count {
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
    println!("Total = {}, paylod(bytes) = {}, throughput = {} messages/s", acked, payload_size, throughput_secs);
    Ok(())
}

#[tokio::main(core_threads = 2)]
async fn main() -> Result<(), Box<dyn Error>> {
    let config: Config = argh::from_env();
    let count = config.count;
    let payload_size = config.payload_size;

    let _server = task::spawn(server());
    tokio::time::delay_for(Duration::from_millis(1)).await;
    client(payload_size, count).await.unwrap();
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
