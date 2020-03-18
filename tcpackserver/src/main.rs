use argh::FromArgs;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::pin;
use tokio::select;
use tokio::stream;
use tokio::stream::StreamExt;
use tokio::task;
use tokio_util::codec::Framed;
use tokio_util::codec::LinesCodec;

use std::error::Error;
use std::time::Instant;

#[derive(FromArgs)]
/// Reach new heights.
struct Config {
    /// size of payload
    #[argh(option, short = 'p', default = "1024")]
    payload_size: usize,

    /// number of messages
    #[argh(option, short = 'n', default = "10000")]
    count: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config: Config = argh::from_env();
    // Next up we create a TCP listener which will listen for incoming
    // connections. This TCP listener is bound to the address we determined
    // above and must be associated with an event loop.
    let mut listener = TcpListener::bind("127.0.0.1:8080").await?;

    task::spawn(async move {
        loop {
            // Asynchronously wait for an inbound socket.
            let (socket, _) = listener.accept().await.unwrap();
            task::spawn(async move {
                let mut frames = Framed::new(socket, LinesCodec::new());
                while let Some(_line) = frames.next().await {
                    frames.get_mut().write_all(b"ack\n").await.unwrap();
                }
            });
        }
    });

    let socket = TcpStream::connect("127.0.0.1:8080").await.unwrap();
    let mut frames = Framed::new(socket, LinesCodec::new());

    let start = Instant::now();
    let stream: Vec<u16> = (0..config.count).collect();
    let stream = stream::iter(stream);

    pin!(stream);
    let mut count = 0;
    loop {
        select! {
            Some(_) = stream.next() => {
                let payload = common::generate_line(config.payload_size);
                frames.get_mut().write_all(payload.as_bytes()).await.unwrap();
            }
            Some(_data) = frames.next() => {
                count += 1;
                if count >= config.count {
                    break;
                }
            }
        }
    }

    let elapsed = start.elapsed();
    let throughput = (config.payload_size * config.count as usize) as u128 / elapsed.as_millis();
    let throughput_secs = throughput * 1000;
    let throughput_secs_mb = throughput_secs / 1024 / 1024;

    println!("throughput = {} MB/s", throughput_secs_mb);
    Ok(())
}
