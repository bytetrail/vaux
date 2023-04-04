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

use crate::codec::{put_utf8, put_var_u32, variable_byte_int_size, PROP_SIZE_U32, PROP_SIZE_U8};

pub use crate::property::PropertyType;

pub use crate::codec::{
    decode, decode_fixed_header, encode, MqttCodecError, Packet, PacketType, QoSLevel, Reason,
};
pub use crate::connack::ConnAck;
pub use crate::connect::Connect;
pub use crate::will::WillMessage;
pub use crate::{
    disconnect::Disconnect, fixed::FixedHeader, 
    pubresp::PubResp,
    subscribe::Subscribe, subscribe::Subscription,
};
use bytes::{BufMut, BytesMut};
use std::collections::HashMap;

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

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct UserPropertyMap {
    map: HashMap<String, Vec<String>>,
}

impl UserPropertyMap {
    pub fn map(&self) -> &HashMap<String, Vec<String>> {
        &self.map
    }

    pub fn set_property(&mut self, key: &str, value: &str) {
        if self.map.contains_key(key) {
            let v = self.map.get_mut(key).unwrap();
            v.clear();
            v.push(value.to_string());
        } else {
            let v: Vec<String> = vec![value.to_string()];
            self.map.insert(key.to_string(), v);
        }
    }

    pub fn add_property(&mut self, key: &str, value: &str) {
        if self.map.contains_key(key) {
            self.map.get_mut(key).unwrap().push(value.to_string());
        } else {
            let v: Vec<String> = vec![value.to_string()];
            self.map.insert(key.to_string(), v);
        }
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.map.contains_key(key)
    }

    pub fn get(&self, key: &str) -> Option<&Vec<String>> {
        self.map.get(key)
    }
}

impl crate::Size for UserPropertyMap {
    fn size(&self) -> u32 {
        let mut remaining: u32 = 0;
        for (key, value) in self.map.iter() {
            let key_len = key.len() as u32 + 2;
            for v in value {
                remaining += key_len + v.len() as u32 + 3;
            }
        }
        remaining
    }

    fn property_size(&self) -> u32 {
        0
    }

    fn payload_size(&self) -> u32 {
        0
    }
}

impl Encode for UserPropertyMap {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
        for (k, value) in self.map.iter() {
            for v in value {
                dest.put_u8(PropertyType::UserProperty as u8);
                put_utf8(k, dest)?;
                put_utf8(v, dest)?;
            }
        }
        Ok(())
    }
}
