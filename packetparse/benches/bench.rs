#![feature(test)]
extern crate test;

use bytes::{Bytes, BytesMut};
use packetparse::{disassemble, Packet};

use test::Bencher;

fn _packets(count: usize, size: usize) -> Vec<Packet> {
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

#[bench]
fn writestream(b: &mut Bencher) {
    let mut packetstream = BytesMut::new();
    b.iter(|| disassemble(packet(1024), &mut packetstream));
    b.bytes = 1024;
}

