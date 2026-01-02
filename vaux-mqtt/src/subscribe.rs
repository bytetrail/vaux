use crate::packet::ControlPacket;
use crate::{
    codec, CodecSize, Decode, Encode, FixedHeader, MqttCodecError, PacketType, PropertyCodecSize,
    PropertyType,
};
use crate::{property::UserProperty, MqttError, MqttVersion, QoSLevel};
use vaux_macro::{CodecSize, Decode, Encode, PropertyCodecSize};

const MIN_SUBSCRIPTION_ID: u32 = 1;
const MAX_SUBSCRIPTION_ID: u32 = 268_435_455;

/// MQTT v5 3.8.3.1 Subscription Options
/// bits 4 and 5 of the subscription options hold the retain handling flag.
/// Retain handling is used to determine how messages published with the
/// retain flag set to ```true``` are handled when the ```SUBSCRIBE``` packet
/// is received.
#[repr(u8)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum RetainHandling {
    #[default]
    Send,
    SendNew,
    None,
}

impl TryFrom<u8> for RetainHandling {
    type Error = MqttCodecError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(RetainHandling::Send),
            0x01 => Ok(RetainHandling::SendNew),
            0x02 => Ok(RetainHandling::None),
            v => Err(MqttCodecError::new(
                format!("Mqttv5 3.8.3.1 invalid retain option: {v}").as_str(),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, CodecSize, PropertyCodecSize)]
pub struct SubAckHeader {
    packet_id: u16,
    #[codec(property_type = "PropertyType::ReasonString")]
    pub reason: Option<String>,
    #[codec(property_type = "PropertyType::UserProperty")]
    pub user_properties: UserProperty,
}

impl Default for SubAckHeader {
    fn default() -> Self {
        Self {
            packet_id: 1,
            reason: None,
            user_properties: UserProperty::new(),
        }
    }
}

/// Subscription represents an MQTT v5 3.8.3 SUBSCRIBE Payload. The Mqtt v5
/// 3.8.3.1 options are represented as individual fields in the struct.
#[derive(Debug, Default, Clone, PartialEq, Eq, Encode, Decode, CodecSize)]
pub struct SubscriptionFilter {
    pub filter: String,
    options: u8,
}

impl SubscriptionFilter {
    /// Create a new Subscription with the given filter and QoSLevel.
    /// The remaining fields are set to their default values.
    pub fn new(filter: String, qos: QoSLevel) -> Self {
        let mut s = Self {
            filter,
            options: 0_u8,
        };
        s.set_qos(qos);
        s
    }

    pub fn qos(&self) -> QoSLevel {
        (self.options & 0b0000_0011)
            .try_into()
            .unwrap_or(QoSLevel::AtMostOnce)
    }

    pub fn set_qos(&mut self, qos: QoSLevel) {
        self.options = (self.options & !0b0000_0011) | (qos as u8 & 0b0000_0011);
    }

    pub fn no_local(&self) -> bool {
        (self.options & 0b0000_0100) != 0
    }

    pub fn set_no_local(&mut self, no_local: bool) {
        self.options = (self.options & !0b0000_0100) | ((no_local as u8) << 2);
    }

    pub fn retain_handling(&self) -> RetainHandling {
        (self.options & 0b0000_1000)
            .try_into()
            .unwrap_or(RetainHandling::Send)
    }

    pub fn set_retain_handling(&mut self, retain: RetainHandling) {
        self.options = (self.options & !0b0000_1000) | ((retain as u8) << 3);
    }
}

#[derive(Default, Debug, Clone, CodecSize, PropertyCodecSize, Encode, Decode, PartialEq, Eq)]
pub struct SubscribeHeader {
    packet_id: u16,
    #[codec(property_type = "PropertyType::SubscriptionIdentifier")]
    #[codec(encode_with = "codec::encode_variable_byte_int_ref")]
    pub subscription_id: Option<u32>,
    #[codec(property_type = "PropertyType::UserProperty")]
    pub props: UserProperty,
}

#[derive(Default, Debug, Clone, CodecSize, Encode, Decode, PartialEq, Eq)]
pub struct SubscribePayload {
    pub filter: Vec<SubscriptionFilter>,
}

pub type Subscribe = ControlPacket<SubscribeHeader, SubscribePayload>;

impl Subscribe {
    pub fn new_subscribe(packet_id: u16) -> Self {
        let fixed_header = FixedHeader::new(PacketType::Subscribe);
        ControlPacket {
            fixed_header,
            variable_header: SubscribeHeader {
                packet_id,
                ..Default::default()
            },
            payload: SubscribePayload { filter: Vec::new() },
        }
    }

    pub fn new_subscribe_with_filter(packet_id: u16, filter: Vec<SubscriptionFilter>) -> Self {
        let fixed_header = FixedHeader::new(PacketType::Subscribe);
        ControlPacket {
            fixed_header,
            variable_header: SubscribeHeader {
                packet_id,
                ..Default::default()
            },
            payload: SubscribePayload { filter },
        }
    }

    pub fn packet_id(&self) -> u16 {
        self.variable_header.packet_id
    }

    pub fn set_packet_id(&mut self, packet_id: u16) {
        // TODO replace with codec error
        assert!(packet_id != 0);
        self.variable_header.packet_id = packet_id;
    }

    pub fn set_subscription_id(&mut self, id: u32) -> Result<(), MqttError> {
        if !(MIN_SUBSCRIPTION_ID..=MAX_SUBSCRIPTION_ID).contains(&id) {
            return Err(MqttError::new_from_spec(
                MqttVersion::V5,
                "3.8.3",
                "subscription ID must be between 1 and 268,435,455",
            ));
        }
        self.variable_header.subscription_id = Some(id);
        Ok(())
    }

    pub fn add_filter(&mut self, filter: SubscriptionFilter) {
        self.payload.filter.push(filter);
    }

    pub fn remove_filter_at(&mut self, index: usize) -> Option<SubscriptionFilter> {
        if index < self.payload.filter.len() {
            Some(self.payload.filter.remove(index))
        } else {
            None
        }
    }

    pub fn remove_filter_with_topic(&mut self, topic: &str) -> Option<SubscriptionFilter> {
        if let Some(pos) = self.payload.filter.iter().position(|f| f.filter == topic) {
            Some(self.payload.filter.remove(pos))
        } else {
            None
        }
    }

    pub fn clear_filters(&mut self) {
        self.payload.filter.clear();
    }
}

//     pub fn new(filter: Vec<SubscriptionFilter>) -> Self {
//         Self { filter }
//     }

//     pub fn new_with_id(id: u32, filter: Vec<SubscriptionFilter>) -> Result<Self, MqttError> {
//         let mut s = Self { filter };
//         Ok(s)
//     }

//     pub fn id(&self) -> Option<u32> {
//         self.id
//     }

// }

// impl From<&Subscribe> for Subscription {
//     fn from(sub: &Subscribe) -> Self {
//         let mut filter = Vec::new();
//         for s in &sub.payload {
//             filter.push(s.clone());
//         }

//         Self {
//             id: Self::id_from_properties(sub.properties()),
//             filter,
//         }
//     }
// }

// impl From<Subscribe> for Subscription {
//     fn from(sub: Subscribe) -> Self {
//         let mut filter = Vec::new();
//         let id = Self::id_from_properties(sub.properties());
//         for s in sub.payload {
//             filter.push(s);
//         }
//         Self { id, filter }
//     }
// }

// /// Create a Subscribe packet from a Subscription. The SUBSCRIBE packet
// /// is created with the Subscription Identifier property set to the id
// /// of the Subscription when that ID is Some.
// impl From<&Subscription> for Subscribe {
//     fn from(sub: &Subscription) -> Self {
//         let mut subscribe = Subscribe::default();
//         if let Some(id) = sub.id {
//             subscribe
//                 .props
//                 .set_codec(Property::SubscriptionIdentifier(id));
//         }
//         for s in &sub.filter {
//             subscribe.add_subscription(s.clone());
//         }
//         subscribe
//     }
// }

// impl From<Subscription> for Subscribe {
//     fn from(sub: Subscription) -> Self {
//         let mut subscribe = Subscribe::default();
//         if let Some(id) = sub.id {
//             subscribe
//                 .props
//                 .set_codec(Property::SubscriptionIdentifier(id));
//         }
//         for s in sub.filter {
//             subscribe.add_subscription(s);
//         }
//         subscribe
//     }
// }

// #[derive(Default, Debug, Clone, PartialEq, Eq, CodecSize, PropertyCodecSize, Encode, Decode)]
// pub struct SubscribeHeader {
//     packet_id: u16,
//     #[codec(property_type = "PropertyType::SubscriptionIdentifier")]
//     pub subscription_id: Option<u32>,
//     #[codec(property_type = "PropertyType::UserProperty")]
//     pub props: UserProperty,
//     //    payload: Vec<SubscriptionFilter>,
// }

// impl Default for Subscribe {
//     fn default() -> Self {
//         Self {
//             packet_id: 0,
//             props: PropertyBundle::new(&SUBSCRIPTION_PROPS),
//             payload: Vec::new(),
//         }
//     }
// }

// impl Subscribe {
//     pub fn new(packet_id: u16, payload: Vec<SubscriptionFilter>) -> Self {
//         Self {
//             packet_id,
//             props: PropertyBundle::new(&SUBSCRIPTION_PROPS),
//             payload,
//         }
//     }

//     pub fn packet_id(&self) -> u16 {
//         self.packet_id
//     }

//     pub fn set_packet_id(&mut self, packet_id: u16) {
//         // TODO replace with codec error
//         assert!(packet_id != 0);
//         self.packet_id = packet_id;
//     }

//     pub fn add_subscription(&mut self, subscription: SubscriptionFilter) {
//         self.payload.push(subscription);
//     }

//     fn encode_payload(&mut self, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
//         if self.payload.is_empty() {
//             return Err(MqttCodecError::new(
//                 "MQTTv5 3.8.3 subscribe payload must exist",
//             ));
//         }
//         for sub in &mut self.payload {
//             sub.encode(dest)?;
//         }
//         Ok(())
//     }
// }
