use crate::connect::Connect;
use crate::property::UserProperty;
use crate::publish::PayloadFormat;
use crate::{
    codec::{self, PropertyCodecSize},
    MqttCodecError, PropertyType, QoSLevel,
};
use vaux_macro::{CodecSize, Decode, Encode, PropertyCodecSize};

#[derive(Debug, Default, Clone, Eq, PartialEq, Encode, Decode, PropertyCodecSize, CodecSize)]
/// MQTT Will message. The Will message name comes from last will and
/// testament. The will message is typically sent under the following
/// conditions when a client disconnects:
/// * IO error or network failure on the server
/// * Client loses contact during defined timeout
/// * Client loses connectivity to the server prior to disconnect
/// * Server closes connection prior to disconnect
///
/// For more information please see
/// <https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc479576982>
pub struct WillHeader {
    #[codec(property_type = "PropertyType::WillDelay")]
    pub will_delay: Option<u32>,
    #[codec(property_type = "PropertyType::PayloadFormat")]
    pub payload_format: Option<PayloadFormat>,
    #[codec(property_type = "PropertyType::MessageExpiry")]
    pub message_expiry: Option<u32>,
    #[codec(property_type = "PropertyType::ContentType")]
    pub content_type: Option<String>,
    #[codec(property_type = "PropertyType::ResponseTopic")]
    pub response_topic: Option<String>,
    #[codec(
        skip_if = "Vec::is_empty",
        property_type = "PropertyType::CorrelationData"
    )]
    pub correlation_data: Vec<u8>,
    #[codec(property_type = "PropertyType::UserProperty")]
    pub user_properties: UserProperty,
}

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub struct WillMessage {
    pub header: WillHeader,
    pub topic: String,
    pub payload: Vec<u8>,
    pub qos: QoSLevel,
    pub retain: bool,
}

impl From<WillMessage> for Connect {
    fn from(will: WillMessage) -> Self {
        let mut connect = Connect::default();
        connect.set_will_message(will);
        connect
    }
}

impl WillMessage {
    pub fn new(topic: String, payload: Vec<u8>, qos: QoSLevel, retain: bool) -> Self {
        WillMessage {
            header: WillHeader::default(),
            topic,
            payload,
            qos,
            retain,
        }
    }
}
