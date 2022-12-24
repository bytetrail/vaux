use crate::{QoSLevel, FixedHeader, UserPropertyMap};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Publish {
    dup: bool,
    qos: QoSLevel,
    retain: bool,
    topic_name: Option<String>,
    topic_alias: Option<u16>,
    response_topic: Option<String>,
    packet_id: Option<u16>,
    payload_format: Option<u8>,
    message_expiry: Option<u32>,
    correlation_data: Option<Vec<u8>>,
    user_props: Option<UserPropertyMap>,
    sub_id: Option<Vec<u32>>,
    content_type: Option<String>,
    payload: Option<Vec<u8>>,
}

impl Publish {
    pub fn new(hdr: FixedHeader) -> Self {
        Publish {
            dup: todo!(),
            qos: todo!(),
            retain: todo!(),
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
        }
    }
}
