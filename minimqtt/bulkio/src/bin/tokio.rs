use argh::FromArgs;

use std::time::{Instant, Duration};

use futures_util::stream;
use futures_util::StreamExt;
use std::io;
use tokio::task;
use tokio::select;
use bulkio::{Publish, Packet, PubAck, Error, mqtt_read, mqtt_write};
use tokio::net::{TcpStream, TcpListener};
use bytes::BytesMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

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

struct Client {
    required_bytes_count: usize,
    buffer: BytesMut,
    writer: BytesMut,
    stream: TcpStream,
}

impl Client {
    fn new(stream: TcpStream) -> Client {
        let buffer = BytesMut::with_capacity(4 * 1024);
        let writer = BytesMut::with_capacity(4 * 1024);

        Client {
            required_bytes_count: 0,
            buffer,
            writer,
            stream,
        }
    }

    async fn next(&mut self) -> Result<Packet, io::Error> {
        loop {
            match mqtt_read(&mut self.buffer) {
                Ok(packet) => return Ok(packet),
                Err(Error::Insufficient(required)) => self.required_bytes_count = required,
                Err(Error::Io(e)) => return Err(e),
            };

            let mut total_read = 0;
            loop {
                let read = self.stream.read_buf(&mut self.buffer).await?;
                if 0 == read {
                    return if self.buffer.is_empty() {
                        Err(io::Error::new(io::ErrorKind::ConnectionReset, "connection reset by peer"))
                    } else {
                        Err(io::Error::new(io::ErrorKind::BrokenPipe, "connection broken by peer"))
                    };
                }

                total_read += read;
                if total_read >= self.required_bytes_count {
                    self.required_bytes_count = 0;
                    break;
                }
            }
        }
    }

    async fn send(&mut self, packet: Packet) -> Result<(), io::Error> {
        mqtt_write(packet, &mut self.writer);
        self.stream.write_all(&self.writer[..]).await?;
        self.writer.clear();
        Ok(())
    }
}

async fn server() -> Result<(), io::Error> {
    let mut listener = TcpListener::bind("127.0.0.1:8080").await?;
    let (stream, _) = listener.accept().await?;
    let mut client = Client::new(stream);
    loop {
        let packet = client.next().await?;
        match packet {
            Packet::Publish(publish) => {
                let ack = Packet::PubAck(PubAck { pkid: publish.pkid });
                client.send(ack).await?;
            }
            Packet::PubAck(_puback) => {}
        };
    }
}

async fn client(config: Config) -> Result<(), io::Error> {
    let socket = TcpStream::connect("127.0.0.1:8080").await.unwrap();
    let mut frames = Client::new(socket);
    let mut stream = stream::iter(packets(config.payload_size, config.count));

    let mut acked = 0;
    let mut sent = 0;
    let start = Instant::now();
    loop {
        select! {
            // sent - acked guard prevents bounded queue deadlock ( assuming 100 packets doesn't
            // cause framed.send() to block )
            Some(packet) = stream.next(), if sent - acked < config.flow_control_size => {
                frames.send(packet).await?;
                sent += 1;
            }
            o = frames.next() => match o.unwrap() {
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
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config: Config = argh::from_env();
    let _server = task::spawn(server());
    tokio::time::delay_for(Duration::from_millis(1)).await;
    client(config).await.unwrap();
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
