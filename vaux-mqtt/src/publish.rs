use crate::{Decode, Encode, FixedHeader, MQTTCodecError, QoSLevel, Remaining, UserPropertyMap};

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
    pub payload_format: Option<u8>,
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
            payload_format: None,
            message_expiry: None,
            correlation_data: None,
            user_props: None,
            sub_id: None,
            content_type: None,
            payload: None,
        })
    }
}

impl Remaining for Publish {
    fn size(&self) -> u32 {
        todo!()
    }

    fn property_remaining(&self) -> Option<u32> {
        todo!()
    }

    fn payload_remaining(&self) -> Option<u32> {
        todo!()
    }
}

impl Encode for Publish {
    fn encode(&self, dest: &mut bytes::BytesMut) -> Result<(), MQTTCodecError> {
        todo!()
    }
}

impl Decode for Publish {
    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<(), MQTTCodecError> {
        todo!()
    }
}
