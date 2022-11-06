use bytes::{BufMut, BytesMut};
use tokio_util::codec::{Decoder, Encoder};
use vaux_mqtt::{
    decode_fixed_header, ConnAck, Connect, Decode, Encode, MQTTCodecError, Packet, PacketType,
};

#[derive(Debug)]
pub struct MQTTCodec;

impl Decoder for MQTTCodec {
    type Item = Packet;
    type Error = MQTTCodecError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        match decode_fixed_header(src) {
            Ok(packet_header) => match packet_header {
                Some(packet_header) => match packet_header.packet_type() {
                    PacketType::PingReq => Ok(Some(Packet::PingRequest(packet_header))),
                    PacketType::PingResp => Ok(Some(Packet::PingResponse(packet_header))),
                    PacketType::Connect => {
                        let mut connect = Connect::default();
                        connect.decode(src)?;
                        Ok(Some(Packet::Connect(connect)))
                    }
                    PacketType::Publish => Ok(None),
                    PacketType::ConnAck => {
                        let mut connack = ConnAck::default();
                        connack.decode(src)?;
                        Ok(Some(Packet::ConnAck(connack)))
                    }
                    _ => Err(MQTTCodecError::new("unsupported packet type")),
                },
                None => Ok(None),
            },
            Err(e) => Err(e),
        }
    }
}

impl Encoder<Packet> for MQTTCodec {
    type Error = MQTTCodecError;

    fn encode(&mut self, packet: Packet, dest: &mut BytesMut) -> Result<(), Self::Error> {
        match packet {
            Packet::Connect(c) => c.encode(dest),
            Packet::ConnAck(c) => c.encode(dest),
            Packet::Disconnect(d) => d.encode(dest),
            Packet::PingRequest(header) | Packet::PingResponse(header) => {
                dest.put_u8(header.packet_type() as u8 | header.flags());
                dest.put_u8(0x_00);
                Ok(())
            }
            _ => return Err(MQTTCodecError::new("unsupported packet type")),
        }?;
        Ok(())
    }
}
