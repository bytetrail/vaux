use crate::codec::{get_bin, get_utf8, put_bin, variable_byte_int_size};
use crate::property::PropertyBundle;
use crate::{put_utf8, Decode, Encode, MqttCodecError, PropertyType, QoSLevel, Size};
use bytes::BytesMut;
use std::collections::HashSet;

lazy_static! {
    static ref SUPPORTED_WILL_PROPS: HashSet<PropertyType> = {
        let mut set = HashSet::new();
        set.insert(PropertyType::WillDelay);
        set.insert(PropertyType::PayloadFormat);
        set.insert(PropertyType::MessageExpiry);
        set.insert(PropertyType::ContentType);
        set.insert(PropertyType::ResponseTopic);
        set.insert(PropertyType::CorrelationData);
        set.insert(PropertyType::UserProperty);
        set
    };
}

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
    pub props: PropertyBundle,
}

impl WillMessage {
    pub fn new(qos: QoSLevel, retain: bool) -> Self {
        WillMessage {
            qos,
            retain,
            topic: "".to_string(),
            payload: Vec::new(),
            props: PropertyBundle::new(SUPPORTED_WILL_PROPS.clone()),
        }
    }
}

impl Decode for WillMessage {
    /// Implementation of decode for will message. The will message decode does
    /// not attempt to decode the flags QOS and Retain as these are present in the
    /// CONNECT flags variable length header prior to the will message properties
    fn decode(&mut self, src: &mut BytesMut) -> Result<(), MqttCodecError> {
        self.props.decode(src)?;
        self.topic = get_utf8(src)?;
        self.payload = get_bin(src)?;
        Ok(())
    }
}

impl Encode for WillMessage {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
        self.props.encode(dest)?;
        put_utf8(&self.topic, dest)?;
        put_bin(&self.payload, dest)?;
        Ok(())
    }
}

impl Size for WillMessage {
    fn size(&self) -> u32 {
        let property_size = self.property_size();
        let property_len = variable_byte_int_size(property_size);
        property_len + property_size + self.payload_size()
    }

    fn property_size(&self) -> u32 {
        self.props.size()
    }

    fn payload_size(&self) -> u32 {
        (self.topic.len() + 2 + self.payload.len() + 2) as u32
    }
}
