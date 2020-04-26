use bytes::{Bytes, Buf, BytesMut, BufMut};

pub mod common;

use common::*;

#[derive(Clone)]
pub struct Packet {
    pub topic: String,
    pub dup: bool,
    pub retain: bool,
    pub qos: u8,
    pub pkid: u16,
    pub payload: Bytes,
}

pub fn assemble(byte1: u8, variable_header_index: usize, mut payload: Bytes) -> Packet {
    let qos = (byte1 & 0b0110) >> 1;
    let dup = (byte1 & 0b1000) != 0;
    let retain = (byte1 & 0b0001) != 0;

    payload.advance(variable_header_index);
    let topic = read_mqtt_string(&mut payload);

    // Packet identifier exists where QoS > 0
    let pkid = match qos {
        0 => 0,
        1 | 2 => payload.get_u16(),
        qos => panic!("Invalid qos = {}", qos)
    };

    if qos != 0 && pkid == 0 {
        panic!("Wrong packet id");
    }

    Packet {
        qos,
        pkid,
        topic,
        payload,
        dup,
        retain,
    }
}

pub fn disassemble(packet: Packet, payload: &mut BytesMut) {
    payload.reserve(packet.topic.len() + packet.payload.len() + 10);
    payload.put_u8(0b0011_0000 | packet.retain as u8 | ((packet.qos as u8) << 1) | ((packet.dup as u8) << 3));
    let mut len = packet.topic.len() + 2 + packet.payload.len();
    if packet.qos != 0 && packet.pkid != 0 {
        len += 2;
    }

    write_remaining_length(payload, len);
    write_mqtt_string(payload, packet.topic.as_str());
    if packet.qos != 0 {
        let pkid = packet.pkid;
        if pkid == 0 {
            panic!("Packet id shouldn't be 0");
        }

        payload.put_u16(pkid);
    }

    payload.put(packet.payload);
}


pub fn next_packet(stream: &mut BytesMut) -> Packet {
    // Read the initial bytes necessary from the stream with out mutating the stream cursor
    let (byte1, remaining_len) = parse_fixed_header(stream);
    let header_len = header_len(remaining_len);
    let len = header_len + remaining_len;
    let variable_header_index = header_len;

    let s = stream.split_to(len);
    assemble(byte1, variable_header_index, s.freeze())
}

