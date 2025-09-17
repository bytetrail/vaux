use std::{collections::HashSet, sync::LazyLock};

use bytes::{Buf, BufMut};

use crate::{
    codec::{get_var_u32, variable_byte_int_size, SIZE_UTF8_STRING},
    property::{PacketProperties, PropertyBundle, UserPropertyMap},
    Decode, Encode, FixedHeader, PacketType, PropertyType, Reason, Size,
};

const UNSUBACK_VAR_HEADER_LEN: u32 = 2;
const UNSUBSCRIBE_VAR_HEADER_LEN: u32 = 2;

static UNSUBACK_PROPS: LazyLock<HashSet<PropertyType>> = LazyLock::new(|| {
    let mut supported = HashSet::new();
    supported.insert(PropertyType::ReasonString);
    supported.insert(PropertyType::UserProperty);
    supported
});

static UNSUBSCRIBE_PROPS: LazyLock<HashSet<PropertyType>> = LazyLock::new(|| {
    let mut supported = HashSet::new();
    supported.insert(PropertyType::UserProperty);
    supported
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsubAck {
    packet_id: u16,
    reason_code: Vec<Reason>,
    props: PropertyBundle,
}

impl Default for UnsubAck {
    fn default() -> Self {
        UnsubAck {
            packet_id: 0,
            reason_code: Vec::new(),
            props: PropertyBundle::new(&UNSUBACK_PROPS),
        }
    }
}

impl PacketProperties for UnsubAck {
    fn properties(&self) -> &PropertyBundle {
        &self.props
    }

    fn properties_mut(&mut self) -> &mut PropertyBundle {
        &mut self.props
    }

    fn set_properties(&mut self, props: PropertyBundle) {
        self.props = props;
    }
}

impl Size for UnsubAck {
    fn size(&self) -> u32 {
        let remaining = self.property_size() + self.payload_size() + UNSUBACK_VAR_HEADER_LEN;
        let len = variable_byte_int_size(remaining);
        len + remaining
    }

    fn property_size(&self) -> u32 {
        self.props.size()
    }

    fn payload_size(&self) -> u32 {
        self.reason_code.len() as u32
    }
}

impl Decode for UnsubAck {
    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<(), crate::MqttCodecError> {
        if src.remaining() < 4 {
            return Err(crate::MqttCodecError {
                reason: "insufficient data to decode packet identifier".to_string(),
                kind: crate::codec::ErrorKind::InsufficientData(4, src.remaining()),
            });
        }
        self.packet_id = src.get_u16();
        println!("packet_id: {}", self.packet_id);
        self.props.decode(src)?;
        while src.remaining() > 0 {
            let code: Reason = src.get_u8().try_into().map_err(|_| crate::MqttCodecError {
                reason: "invalid reason code".to_string(),
                kind: crate::codec::ErrorKind::UnsupportedReason,
            })?;
            self.reason_code.push(code);
        }
        Ok(())
    }
}

impl Encode for UnsubAck {
    fn encode(&self, dest: &mut bytes::BytesMut) -> Result<(), crate::MqttCodecError> {
        let mut header = FixedHeader::new(PacketType::UnsubAck);
        header.remaining = self.size();
        header.encode(dest)?;
        dest.put_u16(self.packet_id);
        self.props.encode(dest)?;
        for reason in &self.reason_code {
            dest.put_u8(*reason as u8);
        }
        Ok(())
    }
}

impl UnsubAck {
    pub fn new(packet_id: u16, reason_codes: Vec<Reason>) -> UnsubAck {
        UnsubAck {
            packet_id,
            reason_code: reason_codes,
            props: PropertyBundle::new(&UNSUBACK_PROPS),
        }
    }

    pub fn packet_id(&self) -> u16 {
        self.packet_id
    }

    pub fn set_packet_id(&mut self, packet_id: u16) {
        self.packet_id = packet_id;
    }

    pub fn reason_code(&self) -> &Vec<Reason> {
        &self.reason_code
    }

    pub fn reason_code_mut(&mut self) -> &mut Vec<Reason> {
        &mut self.reason_code
    }

    pub fn reason(&self) -> String {
        self.props
            .get_property(PropertyType::ReasonString)
            .and_then(|p| {
                if let crate::property::Property::ReasonString(s) = p {
                    Some(s.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default()
    }

    pub fn set_reason(&mut self, reason: String) {
        if reason.is_empty() {
            self.props.clear_property(PropertyType::ReasonString);
            return;
        }
        self.props
            .set_property(crate::property::Property::ReasonString(reason));
    }

    pub fn user_properties(&self) -> &UserPropertyMap {
        self.props.user_properties()
    }

    pub fn user_properties_mut(&mut self) -> &mut UserPropertyMap {
        self.props.user_properties_mut()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsubscribe {
    pub packet_id: u16,
    pub topics: Vec<String>,
    pub props: PropertyBundle,
}

impl Default for Unsubscribe {
    fn default() -> Self {
        Unsubscribe {
            packet_id: 0,
            topics: Vec::new(),
            props: PropertyBundle::new(&UNSUBSCRIBE_PROPS),
        }
    }
}

impl PacketProperties for Unsubscribe {
    fn properties(&self) -> &PropertyBundle {
        &self.props
    }

    fn properties_mut(&mut self) -> &mut PropertyBundle {
        &mut self.props
    }

    fn set_properties(&mut self, props: PropertyBundle) {
        self.props = props;
    }
}

impl Size for Unsubscribe {
    fn size(&self) -> u32 {
        let mut remaining = self.property_size() + UNSUBSCRIBE_VAR_HEADER_LEN;
        for topic in &self.topics {
            remaining += SIZE_UTF8_STRING + topic.len() as u32;
        }
        let len = variable_byte_int_size(remaining);
        len + remaining
    }

    fn property_size(&self) -> u32 {
        self.props.size()
    }

    fn payload_size(&self) -> u32 {
        let mut size = 0;
        for topic in &self.topics {
            size += 2 + topic.len() as u32;
        }
        size
    }
}

impl Decode for Unsubscribe {
    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<(), crate::MqttCodecError> {
        if src.remaining() < 4 {
            return Err(crate::MqttCodecError {
                reason: "insufficient data to decode packet identifier".to_string(),
                kind: crate::codec::ErrorKind::InsufficientData(4, src.remaining()),
            });
        }
        self.packet_id = src.get_u16();
        self.props.decode(src)?;
        while src.remaining() > 0 {
            let topic_len = get_var_u32(src)? as usize;
            if src.remaining() < topic_len {
                return Err(crate::MqttCodecError {
                    reason: "insufficient data to decode topic filter".to_string(),
                    kind: crate::codec::ErrorKind::InsufficientData(topic_len, src.remaining()),
                });
            }
            let topic = String::from_utf8(src.split_to(topic_len).to_vec()).map_err(|_| {
                crate::MqttCodecError {
                    reason: "invalid UTF-8 in topic filter".to_string(),
                    kind: crate::codec::ErrorKind::InvalidUTF8,
                }
            })?;
            self.topics.push(topic);
        }
        Ok(())
    }
}

impl Encode for Unsubscribe {
    fn encode(&self, dest: &mut bytes::BytesMut) -> Result<(), crate::MqttCodecError> {
        let mut header = FixedHeader::new(PacketType::Unsubscribe);
        header.remaining = self.size();
        header.encode(dest)?;
        dest.put_u16(self.packet_id);
        self.props.encode(dest)?;
        for topic in &self.topics {
            if topic.is_empty() {
                return Err(crate::MqttCodecError {
                    reason: "topic filter cannot be empty".to_string(),
                    kind: crate::codec::ErrorKind::MalformedPacket,
                });
            }
            let topic_bytes = topic.as_bytes();
            if topic_bytes.len() > u16::MAX as usize {
                return Err(crate::MqttCodecError {
                    reason: "topic filter too long".to_string(),
                    kind: crate::codec::ErrorKind::MalformedPacket,
                });
            }
            dest.put_u16(topic_bytes.len() as u16);
            dest.put_slice(topic_bytes);
        }
        Ok(())
    }
}

impl Unsubscribe {
    pub fn new(packet_id: u16, topics: Vec<String>) -> Unsubscribe {
        Unsubscribe {
            packet_id,
            topics,
            props: PropertyBundle::new(&UNSUBSCRIBE_PROPS),
        }
    }
}

#[cfg(test)]
mod test {

    #[test]
    fn test_suback_size() {
        use super::*;
        let suback = UnsubAck::new(10, vec![Reason::NoSubscriptionExisted]);
        assert_eq!(suback.size(), 4);
    }

    #[test]
    fn test_suback_size_with_props() {
        use super::*;
        use crate::property::Property;
        let mut suback = UnsubAck::new(10, vec![Reason::NoSubscriptionExisted]);
        suback
            .properties_mut()
            .set_property(Property::ReasonString("test".to_string()));
        assert_eq!(suback.size(), 11);
    }
}
