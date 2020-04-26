use bytes::{Bytes, Buf, BytesMut, BufMut};

pub fn parse_fixed_header(stream: &[u8]) -> (u8, usize) {
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
        panic!("Unexpected Eof")
    }

    (byte1, len)
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
