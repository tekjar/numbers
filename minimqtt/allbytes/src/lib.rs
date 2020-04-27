use bytes::{Bytes, Buf, BytesMut, BufMut};
use thiserror::Error;
use std::io;

mod codec;
pub use codec::*;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Not enough bytes in the io buffer")]
    UnexpectedEof,
    #[error("io failed `{0}`")]
    Io(#[from] io::Error)
}

#[derive(Clone)]
pub struct Publish {
    pub topic: String,
    pub dup: bool,
    pub retain: bool,
    pub qos: u8,
    pub pkid: u16,
    pub payload: Bytes,
}

impl Publish {
    pub fn new(pkid: u16, topic: &str, payload: Vec<u8>) -> Publish {
        Publish {
            topic: topic.to_owned(),
            dup: false,
            retain: false,
            qos: 1,
            pkid,
            payload: Bytes::from(payload)
        }
    }

    pub fn assemble(byte1: u8, variable_header_index: usize, mut payload: Bytes) -> Publish {
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

        Publish {
            qos,
            pkid,
            topic,
            payload,
            dup,
            retain,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PubAck {
    pub pkid: u16,
}

impl PubAck {
    pub fn new(pkid: u16) -> PubAck {
        PubAck {
            pkid
        }
    }

    pub fn assemble(remaining_len: usize, variable_header_index: usize, mut bytes: Bytes) -> PubAck {
        if remaining_len != 2 {
            panic!("Payload size incorrect!!");
        }

        bytes.advance(variable_header_index);
        let pkid = bytes.get_u16();
        let puback = PubAck {
            pkid,
        };

        puback
    }
}

pub enum Packet {
    Publish(Publish),
    PubAck(PubAck)
}

pub fn mqtt_write(packet: Packet, payload: &mut BytesMut) {
    match packet {
        Packet::Publish(packet) => {
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
        Packet::PubAck(packet) => {
            payload.reserve(4);
            payload.put_u8(0x40);
            payload.put_u8(0x02);
            payload.put_u16(packet.pkid);
        }
    }

}


pub fn mqtt_read(stream: &mut BytesMut) -> Result<Packet, Error> {
    // Read the initial bytes necessary from the stream with out mutating the stream cursor
    let (byte1, remaining_len) = parse_fixed_header(stream)?;
    let header_len = header_len(remaining_len);
    let len = header_len + remaining_len;
    let variable_header_index = header_len;
    let control_type = byte1 >> 4;

    // If the current call fails due to insufficient bytes in the stream, after calculating
    // remaining length, we extend the stream
    if stream.len() < len {
        stream.reserve(remaining_len + 2);
        return Err(Error::UnexpectedEof)
    }

    let s = stream.split_to(len);
    Ok(match control_type {
        3 => Packet::Publish(Publish::assemble(byte1, variable_header_index, s.freeze())),
        4 => Packet::PubAck(PubAck::assemble(remaining_len, variable_header_index, s.freeze())),
        typ => panic!("Invalid packet type {}", typ)
    })
}


pub fn parse_fixed_header(stream: &[u8]) -> Result<(u8, usize), Error> {
    if stream.is_empty() {
        panic!("Empty stream")
    }

    let mut mult: usize = 1;
    let mut len: usize = 0;
    let mut done = false;
    let mut stream = stream.iter();

    let byte1 = *stream.next().unwrap();
    for byte in stream {
        let byte = *byte as usize;
        len += (byte & 0x7F) * mult;
        mult *= 0x80;
        if mult > 0x80 * 0x80 * 0x80 * 0x80 {
            panic!("Malformed remaining length")
        }

        done = (byte & 0x80) == 0;
        if done {
            break;
        }
    }

    if !done {
        return Err(Error::UnexpectedEof)
    }

    Ok((byte1, len))
}

pub fn header_len(remaining_len: usize) -> usize {
    if remaining_len >= 2_097_152 {
        4 + 1
    } else if remaining_len >= 16_384 {
        3 + 1
    } else if remaining_len >= 128 {
        2 + 1
    } else {
        1 + 1
    }
}

pub fn read_mqtt_string(stream: &mut Bytes) -> String {
    let len = stream.get_u16() as usize;
    let s = stream.split_to(len);
    match String::from_utf8(s.to_vec()) {
        Ok(v) => v,
        Err(e) => panic!(e)
    }
}

pub(crate) fn write_mqtt_string(stream: &mut BytesMut, string: &str) {
    stream.put_u16(string.len() as u16);
    stream.extend_from_slice(string.as_bytes());
}

pub(crate) fn write_remaining_length(stream: &mut BytesMut, len: usize) {
    if len > 268_435_455 {
        panic!("Payload too long")
    }

    let mut done = false;
    let mut x = len;

    while !done {
        let mut byte = (x % 128) as u8;
        x /= 128;
        if x > 0 {
            byte |= 128;
        }

        stream.put_u8(byte);
        done = x == 0;
    }
}
