use bytes::{Bytes, BytesMut};
use packetparse::{disassemble, Packet};

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};

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

fn packet(size: usize) -> Packet {
    let topic = "hello/mqtt/parsing/speed/test";
    let packet = Packet {
        topic: topic.to_owned(),
        dup: false,
        retain: false,
        qos: 1,
        pkid: 1,
        payload: Bytes::from(vec![1 as u8; size]),
        bytes: Bytes::new(),
    };

    packet
}

fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput-of-writes");
    group.throughput(Throughput::Bytes(1024 as u64));
    group.bench_function("1K write", |b| {
        let mut packetstream = BytesMut::new();
        b.iter(|| disassemble(packet(1024), &mut packetstream))
    });
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
