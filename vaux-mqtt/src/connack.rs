use bytes::{BufMut, BytesMut};
use crate::{Encode, PROP_SIZE_U32, PROP_SIZE_U8, QoSLevel, Sized, UserProperty};
use crate::{FixedHeader, MQTTCodecError, PacketType};
use crate::codec::{encode_variable_len_integer, PROP_SIZE_U16};
use crate::codec::PropertyType::MaxQoS;

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
    retain_avail: bool,
    max_packet_size: Option<u32>,
    assigned_client_id: Option<String>,
    topic_alias_max: Option<u16>,
    user_properties: Option<UserProperty>,
    wildcard_sub_avail: bool,
    sub_id_avail: bool,
    shared_sub_avail: bool,
    server_keep_alive: Option<u16>,
    response_info: Option<String>,
    server_ref: Option<String>,
    auth_method: Option<String>,
    auth_data: Option<Vec<u8>>
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
            retain_avail: true,
            max_packet_size: None,
            assigned_client_id: None,
            topic_alias_max: None,
            user_properties: None,
            wildcard_sub_avail: true,
            sub_id_avail: true,
            shared_sub_avail: true,
            server_keep_alive: None,
            response_info: None,
            server_ref: None,
            auth_method: None,
            auth_data: None,
        }
    }
}

impl crate::Sized for ConnAck {
    fn size(&self) -> u32 {
        let mut remaining = 0;
        // minimum size of ack flags and reason code, 0 byte property length
        remaining = 3;
        // properties
        if !self.expiry_interval.is_none() {
            remaining += PROP_SIZE_U32;
        }
        if self.receive_max != DEFAULT_RECV_MAX {
            remaining += PROP_SIZE_U16;
        }
        if self.max_qos != QoSLevel::AtLeastOnce {
            remaining += PROP_SIZE_U8;
        }
        if !self.retain_avail {
            remaining += PROP_SIZE_U8;
        }
        if !self.max_packet_size.is_none() {
            remaining += PROP_SIZE_U32;
        }
        if let Some(assigned_client_id) = &self.assigned_client_id {
            remaining += 3 + assigned_client_id.len() as u32;
        }
        if !self.topic_alias_max.is_none() {
            remaining += PROP_SIZE_U16;
        }
        if let Some(reason) = &self.reason_str {
            remaining += 2 + reason.len() as u32;
        }
        if let Some(user_properties) = &self.user_properties {
            remaining += user_properties.size();
        }
        if !self.wildcard_sub_avail {
            remaining += PROP_SIZE_U8;
        }
        if !self.sub_id_avail {
            remaining += PROP_SIZE_U8;
        }
        if !self.shared_sub_avail {
            remaining += PROP_SIZE_U8;
        }
        if let Some(keep_alive)=self.server_keep_alive {
            remaining += PROP_SIZE_U16;
        }
        if let Some(response_info) = &self.response_info {
            remaining += 3 + response_info.len() as u32;
        }
        if let Some(auth_method) = &self.auth_method {
            remaining += 3 + auth_method.len() as u32;
        }
        if let Some(auth_data) = &self.auth_data {
            remaining += 3 + auth_data.len() as u32;
        }

        remaining
    }
}

impl Encode for ConnAck {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MQTTCodecError> {
        let cursor_pos = dest.position();
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

#[cfg(test)]
mod test {
    use bytes::BytesMut;
    use crate::{ConnAck, Encode, Reason};
    use super::*;

    /// Minimum length CONNACK return
    /// Byte 1 = packet type + flags
    /// Byte 2 = 1 byte variable byte integer remaining length
    /// Byte 3 = CONNACK flags
    /// Byte 4 = Reason
    /// Byte 5 = 1 byte variable byte integer property length
    const EXPECTED_MIN_CONNACK_LEN: usize = 5;

    #[test]
    fn test_simple_encode() {
        let mut dest = BytesMut::new();
        let connack = ConnAck::new(Reason::Success);
        assert!(connack.encode(&mut dest).is_ok());
        assert_eq!(EXPECTED_MIN_CONNACK_LEN, dest.len());
        assert_eq!(PacketType::ConnAck, PacketType::from(dest[0]));
        // remaining size test
        assert_eq!(3, dest[1]);
        // property length
        assert_eq!(0, dest[4]);
    }

    #[test]
    fn test_encode_expiry_interval() {
        let mut dest = BytesMut::new();
        let mut connack = ConnAck::new(Reason::Success);
        connack.expiry_interval = Some(1000);
        assert!(connack.encode(&mut dest).is_ok());
        assert_eq!(EXPECTED_MIN_CONNACK_LEN+5, dest.len());
    }
}