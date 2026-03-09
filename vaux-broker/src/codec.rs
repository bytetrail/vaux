use bytes::{Buf, BufMut, BytesMut};
use tokio_util::codec::{Decoder, Encoder};
use vaux_mqtt::{
    codec::{
        fixed::{FixedHeader},
        Encode, Decode, CodecSize},
    ConnAck, Connect, Disconnect, MqttCodecError, Packet, PacketType, PingReq, PingResp,
};

#[derive(Debug)]
pub struct MqttCodec;

impl Decoder for MqttCodec {
    type Item = Packet;
    type Error = MqttCodecError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let mut header = FixedHeader::default();
        match header.decode(src) {
            Ok(size_read) => {
                if src.remaining() < size_read as usize {
                    return Ok(None);
                }
                let packet = match header.packet_type {
                    PacketType::Connect => {
                        let mut connect = Connect::new_with_fixed_header(header)?;
                        let _ = connect.decode(src)?;
                        Packet::Connect(Box::new(connect))
                    }
                    PacketType::ConnAck => {
                        let mut connack = ConnAck::new_with_fixed_header(header)?;
                        let _ = connack.decode(src)?;
                        Packet::ConnAck(connack)
                    }
                    PacketType::Disconnect => {
                        let mut disconnect = Disconnect::new_with_fixed_header(header)?;
                        let _ = disconnect.decode(src)?;
                        Packet::Disconnect(disconnect)
                    }
                    PacketType::PingReq => {
                        let mut pingreq = PingReq::new_with_fixed_header(header)?;
                        let _ = pingreq.decode(src)?;
                        Packet::PingRequest(pingreq)
                    }
                    PacketType::PingResp => {
                        let mut pingresp = PingResp::new_with_fixed_header(header)?;
                        let _ = pingresp.decode(src)?;
                        Packet::PingResponse(pingresp)
                    }
                    _ => return Err(MqttCodecError::new("unsupported packet type")),
                };
                Ok(Some(packet))
            }
            Err(e) => Err(e),
        }
    }
}

impl Encoder<Packet> for MqttCodec {
    type Error = MqttCodecError;

    fn encode(&mut self, mut packet: Packet, dest: &mut BytesMut) -> Result<(), Self::Error> {
        packet.encode(dest)
    }
}
