use bytes::{Bytes, Buf, BytesMut, BufMut};
use thiserror::Error;
use std::io;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Not enough bytes in the io buffer")]
    Insufficient(usize),
    #[error("io failed `{0}`")]
    Io(#[from] io::Error)
}

#[derive(Debug, Clone)]
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

    fn assemble(fixed_header: FixedHeader, mut payload: Bytes) -> Publish {
        let qos = (fixed_header.byte1 & 0b0110) >> 1;
        let dup = (fixed_header.byte1 & 0b1000) != 0;
        let retain = (fixed_header.byte1 & 0b0001) != 0;

        payload.advance(fixed_header.header_len);
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

    fn assemble(fixed_header: FixedHeader, mut bytes: Bytes) -> PubAck {
        if fixed_header.remaining_len != 2 {
            panic!("Payload size incorrect!!");
        }

        bytes.advance(fixed_header.header_len);
        let pkid = bytes.get_u16();
        let puback = PubAck {
            pkid,
        };

        puback
    }
}

#[derive(Debug, Clone)]
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

            payload.extend_from_slice(&packet.payload[..]);
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

    // If the current call fails due to insufficient bytes in the stream, after calculating
    // remaining length, we extend the stream
    if stream.len() < len {
        return Err(Error::Insufficient(len))
    }
    // println!("id = {}, want = {}, have = {}", control_type, len, stream.len());

    let fixed_header = FixedHeader {
        byte1,
        header_len,
        remaining_len
    };
    let control_type = byte1 >> 4;

    let s = stream.split_to(len);
    Ok(match control_type {
        3 => Packet::Publish(Publish::assemble(fixed_header, s.freeze())),
        4 => Packet::PubAck(PubAck::assemble(fixed_header, s.freeze())),
        typ => panic!("Invalid packet type {}", typ)
    })
}

struct FixedHeader {
    byte1: u8,
    header_len: usize,
    remaining_len: usize
}

pub fn parse_fixed_header(stream: &[u8]) -> Result<(u8, usize), Error> {
    let stream_len = stream.len();
    if stream_len < 2 {
        return Err(Error::Insufficient(2))
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
        return Err(Error::Insufficient(stream_len + 1))
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

/*
#[cfg(test)]
mod test {
    extern crate test;
    use test::Bencher;
    use bytes::{Bytes, BytesMut};
    use crate::{Publish, mqtt_write, Packet, mqtt_read};

    #[bench]
    fn encode_packets(b: &mut Bencher) {
        let mut payload = BytesMut::new();
        let publish = Publish {
            dup: false,
            qos: 1,
            retain: false,
            topic: "this/is/a/very/big/mqtt/topic/for/testing/perf".to_owned(),
            pkid: 10,
            payload: Bytes::from(vec![1; 1024]),
        };

        let publishes = vec![Packet::Publish(publish.clone()); 2 * 1024 * 1024];
        let mut publishes = publishes.into_iter();
        b.iter(|| {
            mqtt_write(publishes.next().unwrap(), &mut payload)
        });
        b.bytes = 1024;
    }

    #[bench]
    fn decode_packets(b: &mut Bencher) {
        let mut payload = BytesMut::new();
        let publish = Publish {
            dup: false,
            qos: 1,
            retain: false,
            topic: "this/is/a/very/big/mqtt/topic/for/testing/perf".to_owned(),
            pkid: 10,
            payload: Bytes::from(vec![1; 1024]),
        };

        b.iter(|| {
            let mut stream = BytesMut::new();
            (0..1000).for_each(|_v| mqtt_write(Packet::Publish(publish.clone()), &mut stream));
            (0..1000).for_each(|_v| {
                mqtt_read(&mut stream).unwrap();
            });
        });

        b.bytes = 1024 * 1000;
    }
}
*/
