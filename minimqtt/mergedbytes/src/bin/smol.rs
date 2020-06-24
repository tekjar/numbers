use argh::FromArgs;

use std::error::Error;
use std::net::{TcpListener, TcpStream};
use std::time::Instant;

use futures_codec::Framed;
use futures_util::future;
use futures_util::stream;
use futures_util::{SinkExt, StreamExt};
use smol::{self, Async, Task};
use std::{io, thread};
use tokio::select;
use mergedbytes::{Publish, Packet, PubAck};
use mergedbytes::codec::futures::MqttCodec;

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
    let listener = Async::<TcpListener>::bind("127.0.0.1:8080").unwrap();
    let (socket, _) = listener.accept().await?;
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

    println!("Done!!!");
    Ok(())
}

async fn client(payload_size: usize, max_count: usize) -> Result<(), io::Error> {
    let socket = Async::<TcpStream>::connect("127.0.0.1:8080").await?;
    let mut frames = Framed::new(socket, MqttCodec);
    let mut stream = stream::iter(packets(payload_size, max_count));

    let mut acked = 0;
    let mut sent = 0;
    let start = Instant::now();
    loop {
        select! {
            Some(packet) = stream.next(), if sent - acked < 100 => {
                frames.send(packet).await.unwrap();
                sent += 1;
            }
            Some(frame) = frames.next() =>  {
                match frame.unwrap() {
                    Packet::Publish(_publish) => {
                    },
                    Packet::PubAck(_ack) => {
                        acked += 1;
                        if acked >= max_count {
                            break;
                        }
                    }
                }
            }
        }
    }

    let elapsed = start.elapsed();
    let throughput = (acked as usize) as u128 / elapsed.as_millis();
    let throughput_secs = throughput * 1000;
    println!("Total = {}, paylod(bytes) = {}, throughput = {} messages/s", acked, payload_size, throughput_secs);
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let config: Config = argh::from_env();
    let count = config.count;
    let payload_size = config.payload_size;

    // Create a thread pool.
    for _ in 0..4 {
        thread::spawn(|| smol::run(future::pending::<()>()));
    }

    smol::block_on(async {
        let _server = Task::spawn(server());
        client(payload_size, count).await.unwrap();
    });

    Ok(())
}


pub fn packets(size: usize, count: usize) -> Vec<Packet> {
    let mut out = Vec::new();
    for i in 0..count {
        let pkid = (i % 65000) as u16 + 1;
        let payload = vec![i as u8; size];
        let packet = Publish::new(pkid, "hello/mqtt/topic/bytes", payload);
        out.push(Packet::Publish(packet))
    }

    out
}
