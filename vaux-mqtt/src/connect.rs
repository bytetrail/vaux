use crate::{
    codec, property, will::WillHeader, CodecSize, Decode, Encode, MqttCodecError,
    PropertyCodecSize, PropertyType, QoSLevel, WillMessage,
};
// use bytes::BytesMut;
use vaux_macro::{CodecSize, Decode, Encode, PropertyCodecSize};

pub(crate) const CONNECT_FLAG_USERNAME: u8 = 0b_1000_0000;
pub(crate) const CONNECT_FLAG_PASSWORD: u8 = 0b_0100_0000;
pub(crate) const CONNECT_FLAG_WILL_RETAIN: u8 = 0b_0010_0000;
pub(crate) const CONNECT_FLAG_WILL_QOS: u8 = 0b_0001_1000;
pub(crate) const CONNECT_FLAG_WILL: u8 = 0b_0000_0100;
pub(crate) const CONNECT_FLAG_CLEAN_START: u8 = 0b_0000_0010;

const MQTT_PROTOCOL_NAME: &str = "MQTT";
const MQTT_PROTOCOL_VERSION: u8 = 0x05;

#[derive(Default, Debug, Clone, PropertyCodecSize, CodecSize, Encode, Decode, PartialEq, Eq)]
pub struct ConnectHeader {
    protocol_name: String,
    protocol_version: u8,
    connect_flags: u8,
    pub keep_alive: u16,

    #[property(property_type = "PropertyType::SessionExpiryInterval")]
    pub session_expiry_interval: Option<u32>,
    #[property(property_type = "PropertyType::RecvMax")]
    pub receive_maximum: Option<u16>,
    #[property(property_type = "PropertyType::MaxPacketSize")]
    pub max_packet_size: Option<u32>,
    #[property(property_type = "PropertyType::TopicAliasMax")]
    pub topic_alias_maximum: Option<u16>,
    #[property(property_type = "PropertyType::ReqRespInfo")]
    pub request_response_info: Option<bool>,
    #[property(property_type = "PropertyType::ReqProblemInfo")]
    pub request_problem_info: Option<bool>,
    #[property(property_type = "PropertyType::AuthMethod")]
    pub auth_method: Option<String>,
    #[property(property_type = "PropertyType::AuthData")]
    pub auth_data: Vec<u8>,
    #[property(property_type = "PropertyType::UserProperty")]
    pub user_properties: property::UserProperty,
}

//pub will_message: Option<WillMessage>,
//pub username: Option<String>,
//    pub password: Option<Vec<u8>>,
//#[property(property_type = "PropertyType::AuthMethod")]
//pub auth_method: Option<String>,

impl ConnectHeader {
    /// Creates a new ConnectHeader with default values. The protocol name is set to "MQTT"
    /// and the protocol version is set to 5. The protocol version is not configurable in
    /// this implementation.
    ///  Conntect Flags:
    /// |
    pub fn new() -> Self {
        ConnectHeader {
            protocol_name: MQTT_PROTOCOL_NAME.to_string(),
            protocol_version: MQTT_PROTOCOL_VERSION,
            connect_flags: 0,
            keep_alive: 0,
            session_expiry_interval: None,
            receive_maximum: None,
            max_packet_size: None,
            topic_alias_maximum: None,
            request_response_info: None,
            request_problem_info: None,
            auth_method: None,
            auth_data: Vec::new(),
            user_properties: property::UserProperty::new(),
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

    fn set_username(&mut self, username: bool) {
        self.connect_flags = (self.connect_flags & !CONNECT_FLAG_USERNAME) | (username as u8) << 7;
    }

    pub fn password(&self) -> bool {
        (self.connect_flags & CONNECT_FLAG_PASSWORD) != 0
    }

    fn set_password(&mut self, password: bool) {
        self.connect_flags = (self.connect_flags & !CONNECT_FLAG_PASSWORD) | (password as u8) << 6;
    }
}

#[derive(Debug, Clone, Eq, PartialEq, CodecSize, Encode, Decode)]
pub struct ConnectPayload {
    pub client_id: String,
    pub(crate) will_properties: Option<WillHeader>,
    #[codec(skip_if(empty))]
    pub(crate) will_payload: Vec<u8>,
    pub(crate) will_topic: Option<String>,
    username: Option<String>,
    #[codec(skip_if(empty))]
    password: Vec<u8>,
}

impl Default for ConnectPayload {
    fn default() -> Self {
        ConnectPayload {
            client_id: uuid::Uuid::new_v4().to_string(),
            will_properties: None,
            will_payload: Vec::new(),
            will_topic: None,
            username: None,
            password: Vec::new(),
        }
    }
}

pub type Connect = crate::packet::ControlPacket<ConnectHeader, ConnectPayload>;

impl Connect {
    pub fn client_id(&self) -> &str {
        &self.payload().client_id
    }

    pub fn set_client_id(&mut self, client_id: &str) {
        self.payload_mut().client_id = client_id.to_string();
    }

    pub fn session_expiry_interval(&self) -> Option<u32> {
        self.header().session_expiry_interval
    }

    pub fn set_session_expiry_interval(&mut self, interval: u32) {
        if interval == 0 {
            self.header_mut().session_expiry_interval = None;
        } else {
            self.header_mut().session_expiry_interval = Some(interval);
        }
    }

    pub fn keep_alive(&self) -> u16 {
        self.header().keep_alive
    }

    pub fn set_keep_alive(&mut self, keep_alive: u16) {
        self.header_mut().keep_alive = keep_alive;
    }

    pub fn clean_start(&self) -> bool {
        self.header().clean_start()
    }

    pub fn set_clean_start(&mut self, clean_start: bool) {
        self.header_mut().set_clean_start(clean_start);
    }

    pub fn set_username(&mut self, username: Option<String>) {
        match &username {
            Some(_) => self.header_mut().set_username(true),
            None => self.header_mut().set_username(false),
        }
        self.payload_mut().username = username;
    }

    pub fn set_password(&mut self, password: Option<Vec<u8>>) {
        match &password {
            Some(_) => self.header_mut().set_password(true),
            None => self.header_mut().set_password(false),
        }
        self.payload_mut().password = password.unwrap_or_default();
    }

    pub fn set_will_message(&mut self, will: WillMessage) {
        let payload = self.payload_mut();
        payload.will_topic = Some(will.topic);
        payload.will_payload = will.payload;
        payload.will_properties = Some(will.header);

        let header = self.header_mut();

        header.set_will(true);
        header.set_will_qos(will.qos);
        header.set_will_retain(will.retain);
    }
}
