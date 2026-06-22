use crate::{
    codec::{self},
    property,
    will::WillHeader,
    MqttCodecError, PropertyType, QoSLevel, WillMessage,
};
use vaux_macro::packet;

pub(crate) const CONNECT_FLAG_USERNAME: u8 = 0b_1000_0000;
pub(crate) const CONNECT_FLAG_PASSWORD: u8 = 0b_0100_0000;
pub(crate) const CONNECT_FLAG_WILL_RETAIN: u8 = 0b_0010_0000;
pub(crate) const CONNECT_FLAG_WILL_QOS: u8 = 0b_0001_1000;
pub(crate) const CONNECT_FLAG_WILL: u8 = 0b_0000_0100;
pub(crate) const CONNECT_FLAG_CLEAN_START: u8 = 0b_0000_0010;

const MQTT_PROTOCOL_NAME: &str = "MQTT";
const MQTT_PROTOCOL_VERSION: u8 = 0x05;

#[packet(packet_type = "codec::PacketType::Connect")]
#[codec(skip_decode)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connect {
    protocol_name: String,
    protocol_version: u8,
    connect_flags: u8,
    pub keep_alive: u16,

    #[codec(property_type = "PropertyType::SessionExpiryInterval")]
    pub session_expiry_interval: Option<u32>,
    #[codec(property_type = "PropertyType::RecvMax")]
    pub receive_maximum: Option<u16>,
    #[codec(property_type = "PropertyType::MaxPacketSize")]
    pub max_packet_size: Option<u32>,
    #[codec(property_type = "PropertyType::TopicAliasMax")]
    pub topic_alias_maximum: Option<u16>,
    #[codec(property_type = "PropertyType::ReqRespInfo")]
    pub request_response_info: Option<bool>,
    #[codec(property_type = "PropertyType::ReqProblemInfo")]
    pub request_problem_info: Option<bool>,
    #[codec(property_type = "PropertyType::AuthMethod")]
    pub auth_method: Option<String>,
    #[codec(skip_if = "Vec::is_empty", property_type = "PropertyType::AuthData")]
    pub auth_data: Vec<u8>,
    #[codec(property_type = "PropertyType::UserProperty")]
    pub user_properties: property::UserProperty,

    // payload fields
    #[codec(payload_type = "field")]
    pub client_id: String,
    #[codec(payload_type = "field")]
    pub(crate) will_properties: Option<WillHeader>,
    #[codec(payload_type = "field", skip_if = "Vec::is_empty")]
    pub(crate) will_payload: Vec<u8>,
    #[codec(payload_type = "field")]
    pub(crate) will_topic: Option<String>,
    #[codec(skip_if = "String::is_empty", payload_type = "field")]
    username: String,
    #[codec(skip_if = "Vec::is_empty", payload_type = "field")]
    password: Vec<u8>,
}

impl Default for Connect {
    fn default() -> Self {
        Self::new()
    }
}

impl Connect {
    /// Creates a new ConnectHeader with default values. The protocol name is set to "MQTT"
    /// and the protocol version is set to 5. The protocol version is not configurable in
    /// this implementation.
    ///  Conntect Flags:
    /// |
    pub fn new() -> Self {
        Connect {
            fixed_header: codec::FixedHeader::new(codec::PacketType::Connect),
            protocol_name: MQTT_PROTOCOL_NAME.to_string(),
            protocol_version: MQTT_PROTOCOL_VERSION,
            connect_flags: 0,
            keep_alive: 60,
            session_expiry_interval: None,
            receive_maximum: None,
            max_packet_size: None,
            topic_alias_maximum: None,
            request_response_info: None,
            request_problem_info: None,
            auth_method: None,
            auth_data: Vec::new(),
            user_properties: property::UserProperty::new(),
            client_id: uuid::Uuid::new_v4().to_string(),
            will_properties: None,
            will_payload: Vec::new(),
            will_topic: None,
            username: String::new(),
            password: Vec::new(),
        }
    }

    pub fn clean_start(&self) -> bool {
        (self.connect_flags & CONNECT_FLAG_CLEAN_START) != 0
    }

    pub fn set_clean_start(&mut self, clean_start: bool) {
        self.connect_flags =
            (self.connect_flags & !CONNECT_FLAG_CLEAN_START) | (clean_start as u8) << 1;
    }

    pub fn will_retain(&self) -> bool {
        (self.connect_flags & CONNECT_FLAG_WILL_RETAIN) != 0
    }

    pub(crate) fn set_will_retain(&mut self, will_retain: bool) {
        self.connect_flags =
            (self.connect_flags & !CONNECT_FLAG_WILL_RETAIN) | (will_retain as u8) << 5;
    }

    pub fn will_qos(&self) -> Result<QoSLevel, MqttCodecError> {
        ((self.connect_flags & CONNECT_FLAG_WILL_QOS) >> 3).try_into()
    }

    pub(crate) fn set_will_qos(&mut self, qos: QoSLevel) {
        self.connect_flags = (self.connect_flags & !CONNECT_FLAG_WILL_QOS) | ((qos as u8) << 3);
    }

    pub fn will(&self) -> bool {
        (self.connect_flags & CONNECT_FLAG_WILL) != 0
    }

    pub(crate) fn set_will(&mut self, will: bool) {
        self.connect_flags = (self.connect_flags & !CONNECT_FLAG_WILL) | (will as u8) << 2;
    }

    pub fn username(&self) -> bool {
        (self.connect_flags & CONNECT_FLAG_USERNAME) != 0
    }

    pub fn set_username(&mut self, username: Option<String>) {
        if username.is_none() {
            self.connect_flags = self.connect_flags & !CONNECT_FLAG_USERNAME;
        } else {
            self.connect_flags = (self.connect_flags & !CONNECT_FLAG_USERNAME) | (1 as u8) << 7;
        }
        self.username = username.unwrap_or_default();
    }

    pub fn password(&self) -> bool {
        (self.connect_flags & CONNECT_FLAG_PASSWORD) != 0
    }

    pub fn set_password(&mut self, password: Option<Vec<u8>>) {
        if password.is_none() {
            self.connect_flags = self.connect_flags & !CONNECT_FLAG_PASSWORD;
        } else {
            self.connect_flags = (self.connect_flags & !CONNECT_FLAG_PASSWORD) | (1 as u8) << 6;
        }
        self.password = password.unwrap_or_default();
    }

    pub fn set_session_expiry_interval(&mut self, interval: u32) {
        if interval == 0 {
            self.session_expiry_interval = None;
        } else {
            self.session_expiry_interval = Some(interval);
        }
    }

    pub fn will_message(&self) -> Option<WillMessage> {
        if self.will() {
            Some(WillMessage {
                topic: self.will_topic.clone().unwrap_or_default(),
                payload: bytes::Bytes::from(self.will_payload.clone()),
                qos: self.will_qos().unwrap_or(QoSLevel::AtMostOnce),
                retain: self.will_retain(),
                header: self.will_properties.clone().unwrap_or_default(),
            })
        } else {
            None
        }
    }

    pub fn set_will_message(&mut self, will: WillMessage) {
        self.will_topic = Some(will.topic);
        self.will_payload = will.payload.to_vec();
        self.will_properties = Some(will.header);
        self.set_will(true);
        self.set_will_qos(will.qos);
        self.set_will_retain(will.retain);
    }
}

impl codec::Decode for Connect {
    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<usize, MqttCodecError> {
        use bytes::Buf;
        let mut bytes_read = 0_usize;

        // header fields
        bytes_read += self.protocol_name.decode(src)?;
        self.protocol_version = src.get_u8();
        bytes_read += 1;
        self.connect_flags = src.get_u8();
        bytes_read += 1;
        self.keep_alive = src.get_u16();
        bytes_read += 2;

        // properties
        let (property_length, var_bytes_read) = codec::decode_variable_byte_int(src)?;
        let property_length = property_length as usize;
        bytes_read += var_bytes_read;
        let mut property_bytes_read = 0_usize;
        while property_bytes_read < property_length {
            let property_type: PropertyType = src.get_u8().try_into()?;
            property_bytes_read += 1;
            match property_type {
                PropertyType::SessionExpiryInterval => {
                    let mut value = u32::default();
                    property_bytes_read += value.decode(src)?;
                    self.session_expiry_interval = Some(value);
                }
                PropertyType::RecvMax => {
                    self.receive_maximum = Some(src.get_u16());
                    property_bytes_read += 2;
                }
                PropertyType::MaxPacketSize => {
                    let mut value = u32::default();
                    property_bytes_read += value.decode(src)?;
                    self.max_packet_size = Some(value);
                }
                PropertyType::TopicAliasMax => {
                    self.topic_alias_maximum = Some(src.get_u16());
                    property_bytes_read += 2;
                }
                PropertyType::ReqRespInfo => {
                    self.request_response_info = Some(src.get_u8() != 0);
                    property_bytes_read += 1;
                }
                PropertyType::ReqProblemInfo => {
                    self.request_problem_info = Some(src.get_u8() != 0);
                    property_bytes_read += 1;
                }
                PropertyType::AuthMethod => {
                    let mut value = String::default();
                    property_bytes_read += value.decode(src)?;
                    self.auth_method = Some(value);
                }
                PropertyType::AuthData => {
                    let mut value = Vec::new();
                    property_bytes_read += value.decode(src)?;
                    self.auth_data = value;
                }
                PropertyType::UserProperty => {
                    property_bytes_read += self.user_properties.decode(src)?;
                }
                _ => {
                    return Err(codec::MqttCodecError::new_with_kind(
                        format!(
                            "MQTT v5 property type {:?} is not supported for Connect",
                            property_type
                        )
                        .as_str(),
                        codec::ErrorKind::UnsupportedProperty(property_type as u8),
                    ));
                }
            }
        }
        bytes_read += property_bytes_read;

        // payload: client_id (always present)
        bytes_read += self.client_id.decode(src)?;

        // payload: will fields (only if WILL flag set)
        if self.will() {
            let mut will_header = WillHeader::default();
            bytes_read += will_header.decode(src)?;
            self.will_properties = Some(will_header);

            let mut value = Vec::new();
            let var_bytes_read = value.decode(src)?;
            bytes_read += var_bytes_read;
            self.will_payload = value;

            let mut value = String::default();
            bytes_read += value.decode(src)?;
            self.will_topic = Some(value);
        }

        // payload: username (only if USERNAME flag set)
        if self.username() {
            bytes_read += self.username.decode(src)?;
        }

        // payload: password (only if PASSWORD flag set)
        if self.password() {
            let mut value = Vec::new();
            let var_bytes_read = value.decode(src)?;
            bytes_read += var_bytes_read;
            self.password = value;
        }

        Ok(bytes_read)
    }
}
