use std::{collections::HashSet, sync::LazyLock};

use bytes::{Buf, BufMut};

use crate::{
    codec::{get_var_u32, variable_byte_int_size},
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

pub struct UnsubAck {
    packet_id: u16,
    reason_codes: Vec<Reason>,
    props: PropertyBundle,
}

impl Default for UnsubAck {
    fn default() -> Self {
        UnsubAck {
            packet_id: 0,
            reason_codes: Vec::new(),
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
        self.reason_codes.len() as u32
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
        let expected_remaining = get_var_u32(src)?;
        if src.remaining() < expected_remaining as usize {
            return Err(crate::MqttCodecError {
                reason: "insufficient data to decode unsuback".to_string(),
                kind: crate::codec::ErrorKind::InsufficientData(
                    expected_remaining as usize,
                    src.remaining(),
                ),
            });
        }
        if src.remaining() == 0 {
            return Ok(());
        }
        self.props.decode(src)?;
        while src.remaining() > 0 {
            let code: Reason = src.get_u8().try_into().map_err(|_| crate::MqttCodecError {
                reason: "invalid reason code".to_string(),
                kind: crate::codec::ErrorKind::UnsupportedReason,
            })?;
            self.reason_codes.push(code);
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
        for reason in &self.reason_codes {
            dest.put_u8(*reason as u8);
        }
        Ok(())
    }
}

impl UnsubAck {
    pub fn new(packet_id: u16, reason_codes: Vec<Reason>) -> UnsubAck {
        UnsubAck {
            packet_id,
            reason_codes,
            props: PropertyBundle::new(&UNSUBACK_PROPS),
        }
    }

    pub fn packet_id(&self) -> u16 {
        self.packet_id
    }

    pub fn set_packet_id(&mut self, packet_id: u16) {
        self.packet_id = packet_id;
    }

    pub fn reason_codes(&self) -> &Vec<Reason> {
        &self.reason_codes
    }

    pub fn reason_codes_mut(&mut self) -> &mut Vec<Reason> {
        &mut self.reason_codes
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

pub struct Unsubscribe {
    pub packet_id: u16,
    pub topics: Vec<String>,
    pub props: PropertyBundle,
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
