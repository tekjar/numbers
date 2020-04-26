use bytes::{BytesMut, Bytes};
use packetparse::{disassemble, Packet};
use std::time::Instant;
use std::fs::File;
use prost::Message;
use std::io::Write;

fn main() {
    let packets = packets(5 * 1024 * 1024, 1024);
    let mut packetstream = BytesMut::new();

    let guard = pprof::ProfilerGuard::new(100).unwrap();
    let start = Instant::now();
    packets.into_iter().for_each(|packet| {
        disassemble(packet, &mut packetstream)
    });

    report("publishwritethrouthput.pb", packetstream.len() as u64, start, guard);
}

fn packets(count: usize, size: usize) -> Vec<Packet> {
    let mut packets = Vec::new();
    let topic = "hello/mqtt/parsing/speed/test";
    for i in 0..count {
        let packet = Packet {
            topic: topic.to_owned(),
            dup: false,
            retain: false,
            qos: 1,
            pkid: (i % 65000 + 1) as u16,
            payload: Bytes::from(vec![i as u8; size]),
            bytes: Bytes::new(),
        };

        packets.push(packet);
    }

    packets
}

pub fn report(name: &str, size: u64, start: Instant, guard: pprof::ProfilerGuard) {
    let file_size = size / 1024 / 1024;
    let throughput = file_size as u128 * 1000 / start.elapsed().as_millis();
    println!("{}. File size = {}, Throughput = {} MB/s", name, file_size, throughput);

    if let Ok(report) = guard.report().build() {
        let mut file = File::create(name).unwrap();
        let profile = report.pprof().unwrap();

        let mut content = Vec::new();
        profile.encode(&mut content).unwrap();
        file.write_all(&content).unwrap();
    };
}
