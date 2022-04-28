use bytes::{BufMut, BytesMut};
use crate::{Encode, QoSLevel, Sized, UserProperty};
use crate::{FixedHeader, MQTTCodecError, PacketType};
use crate::codec::encode_variable_len_integer;

const DEFAULT_RECV_MAX: u16 = 65535;

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
    reason_str: Option<String>,
    expiry_interval: Option<u32>,
    receive_max: u16,
    max_qos: QoSLevel,
    max_packet_size: Option<u32>,
    assigned_client_id: Option<String>,
    topic_alias_max: Option<u16>,
    user_properties: Option<UserProperty>,
}

impl ConnAck {
    pub fn new(reason: Reason) -> Self {
        ConnAck {
            session_present: false,
            reason,
            reason_str: None,
            expiry_interval: None,
            receive_max: DEFAULT_RECV_MAX,
            max_qos: QoSLevel::AtLeastOnce,
            max_packet_size: None,
            assigned_client_id: None,
            topic_alias_max: None,
            user_properties: None,
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