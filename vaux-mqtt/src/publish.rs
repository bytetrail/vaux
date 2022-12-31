use std::{collections::HashSet, ops::Rem};

use bytes::{Buf, BufMut, BytesMut};

use crate::{
    codec::{
        decode_utf8_string, decode_variable_len_integer, encode_utf8_string,
        variable_byte_int_size, PropertyType, PROP_SIZE_BINARY, PROP_SIZE_U16, PROP_SIZE_U32,
        PROP_SIZE_U8, PROP_SIZE_UTF8_STRING, SIZE_UTF8_STRING, check_property, decode_binary_data,
    },
    Decode, Encode, FixedHeader, MQTTCodecError, QoSLevel, Remaining, UserPropertyMap,
};

const RETAIN_MASK: u8 = 0b_0000_0001;
const DUP_MASK: u8 = 0b_0000_1000;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Publish {
    pub dup: bool,
    pub qos: QoSLevel,
    pub retain: bool,
    pub topic_name: Option<String>,
    pub topic_alias: Option<u16>,
    pub response_topic: Option<String>,
    pub packet_id: Option<u16>,
    pub payload_utf8: bool,
    pub message_expiry: Option<u32>,
    pub correlation_data: Option<Vec<u8>>,
    pub user_props: Option<UserPropertyMap>,
    pub sub_id: Option<Vec<u32>>,
    pub content_type: Option<String>,
    pub payload: Option<Vec<u8>>,
}

impl Publish {
    pub fn new_from_header(hdr: FixedHeader) -> Result<Self, MQTTCodecError> {
        let qos = QoSLevel::try_from(hdr.flags)?;
        let retain = hdr.flags & RETAIN_MASK != 0;
        let dup = hdr.flags & DUP_MASK != 0;
        if dup && qos == QoSLevel::AtMostOnce {
            return Err(MQTTCodecError::new(
                "[MQTT 3.1.1.1] DUP must be 0 for QOS level \"At most once\"",
            ));
        }
        Ok(Publish {
            dup,
            qos,
            retain,
            topic_name: None,
            topic_alias: None,
            response_topic: None,
            packet_id: None,
            payload_utf8: false,
            message_expiry: None,
            correlation_data: None,
            user_props: None,
            sub_id: None,
            content_type: None,
            payload: None,
        })
    }

    pub fn set_packet_id(&mut self, id: u16) -> Result<(), MQTTCodecError> {
        if self.qos == QoSLevel::AtMostOnce {
            return Err(MQTTCodecError::new(
                "MQTTv53.3.2.2 QOS level must not be At Most Once",
            ));
        }
        self.packet_id = Some(id);
        Ok(())
    }

    fn decode_property(
        &mut self,
        property_type: PropertyType,
        src: &mut BytesMut,
    ) -> Result<(), MQTTCodecError> {
        match property_type {
            PropertyType::PayloadFormat => self.payload_utf8 = src.get_u8() == 1,
            PropertyType::MessageExpiry => self.message_expiry = Some(src.get_u32()),
            PropertyType::TopicAlias => self.topic_alias = Some(src.get_u16()),
            PropertyType::ResponseTopic => self.response_topic = Some(decode_utf8_string(src)?),
            PropertyType::CorrelationData => self.correlation_data = Some(decode_binary_data(src)?),
            PropertyType::SubscriptionId => {
                if self.sub_id.is_none() {
                    self.sub_id = Some(Vec::new());
                }
                self.sub_id.as_mut().unwrap().push(decode_variable_len_integer(src));
            }
            PropertyType::ContentType => self.content_type = Some(decode_utf8_string(src)?),            
            prop =>   return Err(MQTTCodecError::new(&format!(
                "MQTTv5 3.3.2.3 unexpected property type value: {}",
                prop
            )))
        }
        Ok(())
    }
}

impl Remaining for Publish {
    fn size(&self) -> u32 {
        let mut remaining = if let Some(topic_name) = self.topic_name.as_ref() {
            SIZE_UTF8_STRING + topic_name.len() as u32
        } else {
            SIZE_UTF8_STRING
        };
        // packet identifier
        remaining += if let QoSLevel::AtMostOnce = self.qos {
            0
        } else {
            2
        };
        let payload_size = self.payload_remaining().unwrap();
        let prop_remaining = self.property_remaining().unwrap();
        remaining + payload_size + prop_remaining + variable_byte_int_size(prop_remaining)
    }

    fn property_remaining(&self) -> Option<u32> {
        let mut remaining = if self.payload_utf8 { PROP_SIZE_U8 } else { 0 };
        remaining += self.message_expiry.map_or(0, |_| PROP_SIZE_U32)
            + self.topic_alias.map_or(0, |_| PROP_SIZE_U16)
            + self
                .response_topic
                .as_ref()
                .map_or(0, |t| PROP_SIZE_UTF8_STRING + t.len() as u32)
            + self
                .correlation_data
                .as_ref()
                .map_or(0, |c| PROP_SIZE_BINARY + c.len() as u32)
            + self.user_props.as_ref().map_or(0, |p| p.size())
            + self
                .content_type
                .as_ref()
                .map_or(0, |c| PROP_SIZE_UTF8_STRING + c.len() as u32);
        // handle multiple subscription identifiers
        if let Some(v) = self.sub_id.as_ref() {
            for id in v {
                remaining += variable_byte_int_size(*id) + 1;
            }
        }
        return Some(remaining);
    }

    fn payload_remaining(&self) -> Option<u32> {
        if let Some(payload) = &self.payload {
            Some(payload.len() as u32)
        } else {
            Some(0)
        }
    }
}

impl Encode for Publish {
    fn encode(&self, dest: &mut bytes::BytesMut) -> Result<(), MQTTCodecError> {
        match self.topic_name.as_ref() {
            Some(topic_name) => encode_utf8_string(topic_name, dest)?,
            None => encode_utf8_string("", dest)?,
        };
        if self.qos != QoSLevel::AtMostOnce {
            if let Some(packet_id) = self.packet_id {
                dest.put_u16(packet_id);
            } else {
                return Err(MQTTCodecError::new(
                    "MQTTv5 3.3.2.2 packet identifier must be included for QOS 1 or 2",
                ));
            }
        }
        Ok(())
    }
}

impl Decode for Publish {
    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<(), MQTTCodecError> {
        let topic_name = decode_utf8_string(src)?;
        if topic_name.len() == 0 {
            self.topic_name = None;
        } else {
            self.topic_name = Some(topic_name);
        }
        if self.qos != QoSLevel::AtMostOnce {
            self.packet_id = Some(src.get_u16());
        }
        let property_len = decode_variable_len_integer(src);
        let payload_len = src.remaining() - property_len as usize;
        let mut properties: HashSet<PropertyType> = HashSet::new();
        while src.remaining() > payload_len {
            match PropertyType::try_from(src.get_u8()) {
                Ok(property_type) => {
                    if property_type != PropertyType::UserProperty {
                        check_property(property_type, &mut properties)?;
                        self.decode_property(property_type, src)?;
                    } else {
                        if self.user_props == None {
                            self.user_props = Some(UserPropertyMap::new());
                        }
                        let property_map = self.user_props.as_mut().unwrap();
                        let key = decode_utf8_string(src)?;
                        let value = decode_utf8_string(src)?;
                        property_map.add_property(&key, &value);
                    }
                }
                Err(_) => return Err(MQTTCodecError::new("MQTTv5 3.3.2.3 invalid property type")),
            };
        }
        if src.remaining() > 0 {
            match src.get(src.len() - src.remaining()..src.remaining()) {
                Some(p) => self.payload = Some(Vec::from(p)),
                None => return Err(MQTTCodecError::new("unable to decode payload")),
            }    
        }
        Ok(())
    }
}
