use criterion::{black_box, criterion_group, criterion_main, Criterion, BatchSize, Throughput};
use bytes::{BytesMut, Bytes};
use basic::{Publish, mqtt_write, Packet, mqtt_read};

fn new_publish(topic_len: usize, payload_size: usize) -> Packet {
    let publish = Publish {
        dup: false,
        qos: 1,
        retain: false,
        topic: "t".repeat(topic_len),
        pkid: 10,
        payload: Bytes::from(vec![1; payload_size]),
    };

    Packet::Publish(publish)
}

fn new_publish_stream(topic_len: usize, payload_size: usize, count: usize) -> BytesMut {
    let mut stream = BytesMut::new();
    let publish = new_publish(topic_len, payload_size);
    (0..count).for_each(|_v| mqtt_write(publish.clone(), &mut stream));
    stream
}

fn encode(publish: Packet) {
    let mut payload = BytesMut::new();
    mqtt_write(publish, &mut payload)
}

fn decode(mut stream: BytesMut, count: usize) {
    (0..count).for_each(|_v| {
        mqtt_read(&mut stream).unwrap();
    });
}

pub fn encoding_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode throughput");
    group.throughput(Throughput::Bytes(1024));
    group.bench_function("encode 10, 1024", move |b| {
        b.iter_batched(|| new_publish(10, 1024), |publish| {
            encode(black_box(publish))
        }, BatchSize::LargeInput)
    });
}

pub fn decoding_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode throughtput");
    let count = 1000;
    group.throughput(Throughput::Bytes(1024 * count as u64));
    group.bench_function("decode 10, 1024, 1000", move |b| {
        b.iter_batched(|| new_publish_stream(10, 1024, count), |stream| {
            black_box(decode(stream, count))
        }, BatchSize::LargeInput)
    });
}

criterion_group!(benches, encoding_benchmark, decoding_benchmark);
criterion_main!(benches);