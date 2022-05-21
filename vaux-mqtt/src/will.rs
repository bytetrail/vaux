use std::collections::HashMap;
use crate::{PROP_SIZE_U32, PROP_SIZE_U8, QoSLevel, Remaining};

const DEFAULT_WILL_DELAY: u32 = 0;

#[derive(Debug, Eq, PartialEq)]
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
    pub user_property: Option<HashMap<String, String>>,
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

impl WillMessage {
    pub fn size(&self) -> u32 {
        let mut remaining = 0;
        if self.delay_interval != DEFAULT_WILL_DELAY {
            remaining += PROP_SIZE_U32;
        }
        if self.payload_utf8 {
            remaining += PROP_SIZE_U8;
        }
        if self.expiry_interval != None {
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
}
