use std::fs::{self, OpenOptions};
use std::io::Write;
use std::time::Instant;
use argh::FromArgs;

#[derive(FromArgs)]
/// Reach new heights.
struct Config {
    /// size of payload
    #[argh(option, short = 'p', default = "1073741824")]
    payload_size: usize,

    /// number of messages
    #[argh(option, short = 'n', default = "10000")]
    count: u16,
}
fn main() {
    let config: Config = argh::from_env();
    let file_name = "/tmp/napkin.txt";

    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(file_name)
        .unwrap();


    let start = Instant::now();
    let payload = common::generate_payload(config.payload_size);
    println!("generating data ............ took {:?} seconds", start.elapsed().as_secs());
    let start = Instant::now();
    file.write_all(&payload).unwrap();
    file.sync_data().unwrap();
    
    let elapsed = start.elapsed();
    let throughput = config.payload_size as u128 / elapsed.as_millis();
    let throughput_secs = throughput * 1000;
    let throughput_secs_mb = throughput_secs / 1024 / 1024;

    fs::remove_file(file_name).unwrap();
    println!("throughput = {} MB/s", throughput_secs_mb);
}
