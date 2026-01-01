use bytes::{BufMut, BytesMut};
use tokio_util::codec::{Decoder, Encoder};
use vaux_mqtt::{
    ConnAck, Connect, Decode, Disconnect, Encode, MqttCodecError, Packet, PacketType, codec::{PingReqCtrl, PingRespCtrl}, decode_fixed_header
};

#[derive(Debug)]
pub struct MqttCodec;

impl Decoder for MqttCodec {
    type Item = Packet;
    type Error = MqttCodecError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        match decode_fixed_header(src) {
            Ok(packet_header) => match packet_header {
                Some(packet_header) => match packet_header.packet_type() {
                    PacketType::PingReq => {
                        Ok(Some(Packet::PingRequest(PingReqCtrl::new(packet_header))))
                    }
                    PacketType::PingResp => {
                        Ok(Some(Packet::PingResponse(PingRespCtrl::new(packet_header))))
                    }
                    PacketType::Connect => {
                        let mut connect = Connect::default();
                        connect.decode(src)?;
                        Ok(Some(Packet::Connect(Box::new(connect))))
                    }
                    PacketType::Publish => Ok(None),
                    PacketType::ConnAck => {
                        let mut connack = ConnAck::default();
                        connack.decode(src)?;
                        Ok(Some(Packet::ConnAck(connack)))
                    }
                    PacketType::Disconnect => {
                        let mut disconnect = Disconnect::default();
                        disconnect.decode(src)?;
                        Ok(Some(Packet::Disconnect(disconnect)))
                    }
                    _ => Err(MqttCodecError::new("unsupported packet type")),
                },
                None => Ok(None),
            },
            Err(e) => Err(e),
        }
    }
}

impl Encoder<Packet> for MqttCodec {
    type Error = MqttCodecError;

    fn encode(&mut self, packet: Packet, dest: &mut BytesMut) -> Result<(), Self::Error> {
        match packet {
            Packet::Connect(c) => c.encode(dest),
            Packet::ConnAck(c) => c.encode(dest),
            Packet::Disconnect(d) => d.encode(dest),
            Packet::PingRequest(header) | Packet::PingResponse(header) => header.encode(dest)?,
            _ => return Err(MqttCodecError::new("unsupported packet type")),
        }?;
        Ok(())
    }
}
