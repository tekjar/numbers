use argh::FromArgs;

use std::time::Instant;
use futures_util::{future, AsyncWriteExt, AsyncReadExt};
use smol::{self, Async, Task};
use std::{io, thread};
use tokio::select;
use bulkio::{Publish, Packet, PubAck, Error, mqtt_read, mqtt_write};
use bytes::BytesMut;
use std::net::{TcpStream, TcpListener};
use std::vec::IntoIter;

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
    pending: usize,
    read: BytesMut,
    write: BytesMut,
    stream: Async<TcpStream>,
}

impl Client {
    fn new(stream: Async<TcpStream>) -> Client {
        let buffer = BytesMut::with_capacity(4 * 1024);
        let writer = BytesMut::with_capacity(4 * 1024);

        Client {
            pending: 0,
            read: buffer,
            write: writer,
            stream,
        }
    }

    async fn next(&mut self) -> Result<Vec<Packet>, io::Error> {
        let mut out = Vec::with_capacity(10);

        loop {
            match mqtt_read(&mut self.read) {
                Ok(packet) => {
                    out.push(packet);
                    if out.len() >= 10 { break; }
                    continue;
                }
                Err(Error::Insufficient(required)) => {
                    self.pending = required;
                    if out.len() > 0 {
                        break;
                    }
                }
                Err(Error::Io(e)) => return Err(e),
            };

            let mut total_read = 0;
            let mut buf = [0u8; 1024];
            loop {
                let read = self.stream.read(&mut buf).await?;
                if 0 == read {
                    return if self.read.is_empty() {
                        Err(io::Error::new(io::ErrorKind::ConnectionReset, "connection reset by peer"))
                    } else {
                        Err(io::Error::new(io::ErrorKind::BrokenPipe, "connection broken by peer"))
                    };
                }

                self.read.extend_from_slice(&buf[..read]);
                total_read += read;
                if total_read >= self.pending {
                    self.pending = 0;
                    break;
                }
            }
        }

        Ok(out)
    }

    fn store(&mut self, packet: Packet) -> Result<(), io::Error> {
        mqtt_write(packet, &mut self.write);
        Ok(())
    }

    async fn flush(&mut self) -> Result<(), io::Error> {
        self.stream.write_all(&self.write[..]).await?;
        self.write.clear();
        Ok(())
    }

    /*
    async fn send(&mut self, packet: Packet) -> Result<(), io::Error> {
        mqtt_write(packet, &mut self.write);
        self.stream.write_all(&self.write[..]).await?;
        self.write.clear();
        Ok(())
    }
    */
}

async fn server() -> Result<(), io::Error> {
    let listener = Async::<TcpListener>::bind("127.0.0.1:8080").unwrap();
    let (socket, _) = listener.accept().await?;

    let mut client = Client::new(socket);
    loop {
        let packets = client.next().await?;
        for packet in packets {
            match packet {
                Packet::Publish(publish) => {
                    client.store(Packet::PubAck(PubAck { pkid: publish.pkid })).unwrap();
                }
                Packet::PubAck(_puback) => {}
            };
        }

        client.flush().await?;
    }
}

async fn client(config: Config) -> Result<(), io::Error> {
    let socket = Async::<TcpStream>::connect("127.0.0.1:8080").await?;
    let mut frames = Client::new(socket);
    let mut stream = Reader::new(packets(config.payload_size, config.count).into_iter());

    let mut acked = 0;
    let mut sent = 0;
    let start = Instant::now();

    'main: loop {
        select! {
            // sent - acked guard prevents bounded queue deadlock ( assuming 100 packets doesn't
            // cause framed.send() to block )
            Some(packets) = stream.next(), if sent - acked < config.flow_control_size => {
                let count = packets.len();
                for packet in packets {
                    frames.store(packet).unwrap();
                }

                frames.flush().await.unwrap();
                sent += count;
            }
            o = frames.next() => {
                let packets = o.unwrap();
                for packet in packets {
                    match packet {
                        Packet::Publish(_publish) => (),
                        Packet::PubAck(_ack) => {
                            acked += 1;
                            if acked >= config.count {
                                break 'main;
                            }
                        },
                    }
                }
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
    println!("Id = smol, Total = {}, Payload size (bytes) = {}, Flow control window len = {}, Throughput (messages/sec) = {}", acked, config.payload_size, config.flow_control_size, throughput_secs);
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config: Config = argh::from_env();

    // Create a thread pool.
    for _ in 0..4 {
        thread::spawn(|| smol::run(future::pending::<()>()));
    }

    smol::block_on(async {
        let _server = Task::spawn(server());
        client(config).await.unwrap();
    });

    Ok(())
}

struct Reader {
    stream: IntoIter<Packet>,
    packets: Vec<Packet>
}

impl Reader {
    fn new(stream: IntoIter<Packet>) -> Reader {
        Reader {
            stream,
            packets: Vec::with_capacity(10)
        }
    }

    async fn next(&mut self) -> Option<Vec<Packet>> {
        for _i in 0..10 {
            if let Some(packet) = self.stream.next() {
                self.packets.push(packet);
            }
        }

        let o = self.packets.split_off(0);
        if o.len() != 0 {
            Some(o)
        } else {
            None
        }
    }
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
