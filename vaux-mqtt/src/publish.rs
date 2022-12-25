use crate::{QoSLevel, FixedHeader, UserPropertyMap, MQTTCodecError};


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
