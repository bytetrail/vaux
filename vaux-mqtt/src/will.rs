use crate::codec::{check_property, get_bin, get_utf8, get_var_u32, put_bin};
use crate::{
    put_utf8, put_var_u32, Decode, Encode, MqttCodecError, PropertyType, QoSLevel, Size,
    UserPropertyMap, PROP_SIZE_U32, PROP_SIZE_U8,
};
use bytes::{Buf, BufMut, BytesMut};
use std::collections::HashSet;

const DEFAULT_WILL_DELAY: u32 = 0;

#[derive(Debug, Clone, Eq, PartialEq)]
/// MQTT Will message. The Will message name comes from last will and
/// testament. The will message is typically sent under the following
/// conditions when a client disconnects:
/// * IO error or network failure on the server
/// * Client loses contact during defined timeout
/// * Client loses connectivity to the server prior to disconnect
/// * Server closes connection prior to disconnect
/// For more information please see
/// <https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc479576982>
pub struct WillMessage {
    pub qos: QoSLevel,
    pub retain: bool,
    pub topic: String,
    pub payload: Vec<u8>,
    pub delay_interval: u32,
    pub expiry_interval: Option<u32>,
    pub payload_utf8: bool,
    pub content_type: Option<String>,
    pub response_topic: Option<String>,
    pub is_request: bool,
    pub correlation_data: Option<Vec<u8>>,
    pub user_property: Option<UserPropertyMap>,
}

impl WillMessage {
    pub fn new(qos: QoSLevel, retain: bool) -> Self {
        WillMessage {
            qos,
            retain,
            topic: "".to_string(),
            payload: Vec::new(),
            delay_interval: 0,
            expiry_interval: None,
            payload_utf8: false,
            content_type: None,
            response_topic: None,
            is_request: false,
            correlation_data: None,
            user_property: None,
        }
    }
}

impl Decode for WillMessage {
    /// Implementation of decode for will message. The will message decode does
    /// not attempt to decode the flags QOS and Retain as these are present in the
    /// CONNECT flags variable length header prior to the will message properties
    fn decode(&mut self, src: &mut BytesMut) -> Result<(), MqttCodecError> {
        let prop_size = get_var_u32(src);
        let read_until = src.remaining() - prop_size as usize;
        let mut properties: HashSet<PropertyType> = HashSet::new();
        while src.remaining() > read_until {
            match PropertyType::try_from(src.get_u8()) {
                Ok(property_type) => match property_type {
                    PropertyType::WillDelay => {
                        check_property(PropertyType::WillDelay, &mut properties)?;
                        self.delay_interval = src.get_u32();
                    }
                    PropertyType::PayloadFormat => {
                        check_property(PropertyType::PayloadFormat, &mut properties)?;
                        match src.get_u8() {
                            0 => self.payload_utf8 = false,
                            1 => self.payload_utf8 = true,
                            err => {
                                return Err(MqttCodecError::new(&format!(
                                    "unexpected will message payload format value: {}",
                                    err
                                )))
                            }
                        }
                    }
                    PropertyType::MessageExpiry => {
                        check_property(PropertyType::MessageExpiry, &mut properties)?;
                        self.expiry_interval = Some(src.get_u32());
                    }
                    PropertyType::ContentType => {
                        check_property(PropertyType::ContentType, &mut properties)?;
                        self.content_type = Some(get_utf8(src)?);
                    }
                    PropertyType::ResponseTopic => {
                        check_property(PropertyType::ResponseTopic, &mut properties)?;
                        self.response_topic = Some(get_utf8(src)?);
                        self.is_request = true;
                    }
                    PropertyType::CorrelationData => {
                        check_property(PropertyType::CorrelationData, &mut properties)?;
                        self.correlation_data = Some(get_bin(src)?);
                    }
                    PropertyType::UserProperty => {
                        if self.user_property.is_none() {
                            self.user_property = Some(UserPropertyMap::default());
                        }
                        let property_map = self.user_property.as_mut().unwrap();
                        let key = get_utf8(src)?;
                        let value = get_utf8(src)?;
                        property_map.add_property(&key, &value);
                    }
                    err => {
                        return Err(MqttCodecError::new(&format!(
                            "unexpected will property id: {}",
                            err
                        )))
                    }
                },
                Err(e) => {
                    return Err(MqttCodecError::new(&format!(
                        "unknown property type: {:?}",
                        e
                    )))
                }
            };
        }
        self.topic = get_utf8(src)?;
        self.payload = get_bin(src)?;
        Ok(())
    }
}

impl Encode for WillMessage {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
        let property_length = self.property_size();
        put_var_u32(property_length, dest);
        if self.delay_interval != 0 {
            dest.put_u8(PropertyType::WillDelay as u8);
            dest.put_u32(self.delay_interval);
        }
        if self.payload_utf8 {
            dest.put_u8(PropertyType::PayloadFormat as u8);
            dest.put_u8(1);
        }
        if let Some(expiry) = self.expiry_interval {
            dest.put_u8(PropertyType::MessageExpiry as u8);
            dest.put_u32(expiry);
        }
        if let Some(content_type) = &self.content_type {
            dest.put_u8(PropertyType::ContentType as u8);
            put_utf8(content_type, dest)?;
        }
        if let Some(response_topic) = &self.response_topic {
            dest.put_u8(PropertyType::ResponseTopic as u8);
            put_utf8(response_topic, dest)?;
        }
        if let Some(correlation_data) = &self.correlation_data {
            dest.put_u8(PropertyType::CorrelationData as u8);
            put_bin(correlation_data, dest)?;
        }
        if let Some(user_properties) = &self.user_property {
            user_properties.encode(dest)?;
        }
        put_utf8(&self.topic, dest)?;
        put_bin(&self.payload, dest)?;
        Ok(())
    }
}

impl Size for WillMessage {
    fn size(&self) -> u32 {
        self.property_size() + self.payload_size()
    }

    fn property_size(&self) -> u32 {
        let mut remaining = 0;
        if self.delay_interval != DEFAULT_WILL_DELAY {
            remaining += PROP_SIZE_U32;
        }
        if self.payload_utf8 {
            remaining += PROP_SIZE_U8;
        }
        if self.expiry_interval.is_none() {
            remaining += PROP_SIZE_U32;
        }
        if let Some(content_type) = &self.content_type {
            remaining += content_type.len() as u32 + 3;
        }
        if let Some(response_topic) = &self.response_topic {
            remaining += response_topic.len() as u32 + 3;
        }
        if let Some(correlation_data) = &self.correlation_data {
            remaining += correlation_data.len() as u32 + 3;
        }
        if let Some(user_property) = &self.user_property {
            remaining += user_property.size();
        }
        remaining += crate::variable_byte_int_size(remaining);
        remaining
    }

    fn payload_size(&self) -> u32 {
        (self.topic.len() + 2 + self.payload.len() + 2) as u32
    }
}
