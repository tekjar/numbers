use argh::FromArgs;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::pin;
use tokio::select;
use tokio::stream;
use tokio::stream::StreamExt;
use tokio::task;
use tokio_util::codec::Framed;
use tokio_util::codec::LinesCodec;
use futures_util::SinkExt;

use std::error::Error;
use std::time::{Instant, Duration};
use std::io;

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

async fn server() -> Result<(), io::Error> {
    let mut listener = TcpListener::bind("127.0.0.1:8080").await?;

    loop {
        let (socket, _) = listener.accept().await?;
        task::spawn(async move {
            let mut frames = Framed::new(socket, LinesCodec::new());
            while let Some(_line) = frames.next().await {
                frames.send("ack".to_owned()).await.unwrap();
            }
        });
    }
}

async fn client(payload_size: usize, max_count: usize) -> Result<(), io::Error> {
    let socket = TcpStream::connect("127.0.0.1:8080").await.unwrap();
    let mut frames = Framed::new(socket, LinesCodec::new());

    let stream: Vec<usize> = (0..max_count).collect();
    let stream = stream::iter(stream);

    pin!(stream);
    let mut count = 0;
    let payload = common::generate_string(payload_size);

    loop {
        select! {
            Some(_) = stream.next() => {
                frames.send(payload.clone()).await.unwrap();
            }
            Some(_data) = frames.next() => {
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

    task::spawn(async move {
        server().await.unwrap();
    });

    tokio::time::delay_for(Duration::from_millis(1)).await;
    let start = Instant::now();
    client(payload_size, count).await.unwrap();

    let elapsed = start.elapsed();
    let throughput = (config.payload_size * config.count as usize) as u128 / elapsed.as_millis();
    let throughput_secs = throughput * 1000;
    let throughput_secs_mb = throughput_secs / 1024 / 1024;
    println!("throughput = {} MB/s", throughput_secs_mb);
    Ok(())
}
