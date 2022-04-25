use bytes::{BufMut, BytesMut};
use crate::{Encode, Sized};
use crate::{FixedHeader, MQTTCodecError, PacketType};
use crate::codec::encode_variable_len_integer;

#[repr(u8)]
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum Reason {
    Success,
    UnspecifiedErr = 0x80,
    MalformedPacket,
    ProtocolErr,
    ImplementationErr,
    UnsupportedProtocolVersion,
    InvalidClientId,
    AuthenticationErr,
    Unauthorized,
    ServerUnavailable,
    ServerBusy,
    Banned,
    AuthMethodErr = 0x8c,
    InvalidTopicName = 0x90,
    PacketTooLarge = 0x95,
    QuotaExceeded = 0x97,
    PayloadFormatErr = 0x99,
    RetainNotSupported,
    QoSNotSupported,
    UseDiffServer,
    ServerMoved,
    ConnRateExceeded = 0x9f
}


#[derive(Debug, Eq, PartialEq)]
pub struct ConnAck {
    session_present: bool,
    reason: Reason,
}

impl ConnAck {
    pub fn new(reason: Reason) -> Self {
        ConnAck {
            session_present: false,
            reason,
        }
    }
}

impl crate::Sized for ConnAck {
    fn size(&self) -> u32 {
        let mut size = 0;
        // minimum size of ack flags and reason code, 0 byte property length
        size = 3;
        size
    }
}

impl Encode for ConnAck {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MQTTCodecError> {
        let mut header = FixedHeader::new(PacketType::ConnAck);
        header.set_remaining(self.size());
        header.encode(dest);
        let connack_flag: u8 = if self.session_present { 0x01 } else { 0x00 };
        dest.put_u8(connack_flag);
        dest.put_u8(self.reason as u8);
        encode_variable_len_integer(0, dest);
        Ok(())
    }
}