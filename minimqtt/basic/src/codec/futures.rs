use bytes::BytesMut;
use futures_codec::{Decoder, Encoder};
use crate::{Error, mqtt_read, mqtt_write, Packet};

pub struct MqttCodec;

impl Decoder for MqttCodec {
    type Item = Packet;
    type Error = crate::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Packet>, crate::Error> {
        // `decode` might be called with `buf.len == 0`. We should return Ok(None)
        // if buf.len() < 2 {
        //     return Ok(None);
        // }

        // Find ways to reserve `buf` better to optimize allocations
        let packet = match mqtt_read(buf) {
            Ok(len) => len,
            Err(Error::UnexpectedEof)  => return Ok(None),
            Err(e) => return Err(e),
        };

        Ok(Some(packet))
    }
}

impl Encoder for MqttCodec {
    type Item = Packet;
    type Error = crate::Error;

    fn encode(&mut self, packet: Packet, buf: &mut BytesMut) -> Result<(), crate::Error> {
        mqtt_write(packet, buf);
        Ok(())
    }
}