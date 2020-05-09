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
    #[argh(option, short = 'p', default = "1024")]
    payload_size: usize,
    /// number of messages
    #[argh(option, short = 'n', default = "10*1024*1024")]
    count: usize,
}

async fn server() -> Result<(), io::Error> {
    let listener = Async::<TcpListener>::bind("127.0.0.1:8080").unwrap();
    let (socket, _) = listener.accept().await?;
    let mut frames = Framed::new(socket, MqttCodec);
    while let Some(packet) = frames.next().await {
        match packet.unwrap() {
            Packet::Publish(publish) => {
                let publish = Packet::Publish(publish);
                frames.send(publish).await.unwrap();
                frames.flush().await.unwrap();
            }
            Packet::PubAck(_puback) => continue
        };
    }

    println!("Done!!!");
    Ok(())
}

async fn client(payload_size: usize, max_count: usize) -> Result<(), io::Error> {
    let socket = Async::<TcpStream>::connect("127.0.0.1:8080").await?;
    let mut frames = Framed::new(socket, MqttCodec);
    let mut stream = stream::iter(packets(payload_size, max_count));

    let mut count = 0;
    let start = Instant::now();
    let mut done = false;
    loop {
        select! {
            packet = stream.next(), if !done => {
                let packet = match packet {
                    Some(packet) => packet,
                    None => {
                        done = true;
                        dbg!("done with the stream");
                        continue
                    }
                };
                frames.send(packet).await.unwrap();
            }
            frame = frames.next() =>  {
                let frame = match frame {
                    Some(f) => f,
                    None => {
                        dbg!("done with network");
                        break
                    }
                };

                match frame.unwrap() {
                    Packet::Publish(_publish) => {
                        count += 1;
                        if count >= max_count {
                            break;
                        }
                    },
                    Packet::PubAck(_ack) => {

                    }
                }
            }
        }
    }

    let elapsed = start.elapsed();
    let throughput = (payload_size * count as usize) as u128 / elapsed.as_millis();
    let throughput_secs = throughput * 1000;
    let throughput_secs_mb = throughput_secs / 1024 / 1024;
    println!("throughput = {} MB/s", throughput_secs_mb);

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
