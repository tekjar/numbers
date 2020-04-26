use argh::FromArgs;

use std::error::Error;
use std::net::{TcpListener, TcpStream};
use std::time::Instant;

use futures_codec::{Framed, LinesCodec};
use futures_util::future;
use futures_util::stream;
use futures_util::{SinkExt, StreamExt};
use smol::{self, Async, Task};
use std::{io, thread};
use tokio::select;

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
    let listener = Async::<TcpListener>::bind("127.0.0.1:8080").unwrap();

    loop {
        let (socket, _) = listener.accept().await?;
        Task::spawn(async move {
            let mut frames = Framed::new(socket, LinesCodec);
            while let Some(_line) = frames.next().await {
                frames.send("ack\n".to_owned()).await.unwrap();
            }
        })
        .await;
    }
}

async fn client(payload_size: usize, max_count: usize) -> Result<(), io::Error> {
    let socket = Async::<TcpStream>::connect("127.0.0.1:8080").await?;
    let frames = Framed::new(socket, LinesCodec);
    let stream: Vec<usize> = (0..max_count).collect();
    let stream = stream::iter(stream);

    let mut count = 0;
    let mut frames = frames.fuse();
    let mut stream = stream.fuse();
    let payload = common::generate_string(payload_size) + "\n";
    loop {
        select! {
            Some(_) = stream.next() => {
                frames.send(payload.clone()).await?;
            }
            Some(o) = frames.next() => {
                count += 1;
                if count >= max_count {
                    break;
                }
            }
        }
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let config: Config = argh::from_env();
    let count = config.count;
    let payload_size = config.payload_size;

    // Create a thread pool.
    for _ in 0..2 {
        thread::spawn(|| smol::run(future::pending::<()>()));
    }

    smol::block_on(async {
        let _server = Task::spawn(server());

        let start = Instant::now();
        client(payload_size, count).await.unwrap();

        let elapsed = start.elapsed();
        let throughput =
            (config.payload_size * config.count as usize) as u128 / elapsed.as_millis();
        let throughput_secs = throughput * 1000;
        let throughput_secs_mb = throughput_secs / 1024 / 1024;

        println!("throughput = {} MB/s", throughput_secs_mb);
    });
    Ok(())
}
