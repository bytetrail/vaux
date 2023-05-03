pub mod codec;
pub mod connack;
pub mod connect;
pub mod disconnect;
pub mod fixed;
pub mod property;
pub mod publish;
pub mod pubresp;
pub mod subscribe;
pub mod test;
mod will;

use crate::codec::{put_utf8, variable_byte_int_size};

pub use crate::property::PropertyType;

pub use crate::codec::{
    decode, decode_fixed_header, encode, MqttCodecError, Packet, PacketType, QoSLevel, Reason,
};
pub use crate::connack::ConnAck;
pub use crate::connect::Connect;
pub use crate::will::WillMessage;
pub use crate::{
    disconnect::Disconnect, fixed::FixedHeader, pubresp::PubResp, subscribe::Subscribe,
    subscribe::Subscription,
};
use bytes::BytesMut;
#[macro_use]
extern crate lazy_static;

pub trait Size {
    fn size(&self) -> u32;
    fn property_size(&self) -> u32;
    fn payload_size(&self) -> u32;
}

pub trait Encode: Size {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError>;
}

pub trait Decode {
    fn decode(&mut self, src: &mut BytesMut) -> Result<(), MqttCodecError>;
}
