use crate::codec::Reason;
use crate::property::{Property, PropertyBundle};
use crate::{Decode, Encode, PropertyType, Size};
use crate::{FixedHeader, MqttCodecError, PacketType};
use bytes::{Buf, BufMut, BytesMut};
use std::collections::HashSet;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ConnAck {
    pub session_present: bool,
    reason: Reason,
    properties: PropertyBundle,
}

impl ConnAck {
    fn allowed_properties() -> HashSet<PropertyType> {
        let mut allowed = HashSet::new();
        allowed.insert(PropertyType::SessionExpiryInterval);
        allowed.insert(PropertyType::RecvMax);
        allowed.insert(PropertyType::MaxQoS);
        allowed.insert(PropertyType::RetainAvail);
        allowed.insert(PropertyType::MaxPacketSize);
        allowed.insert(PropertyType::AssignedClientId);
        allowed.insert(PropertyType::TopicAliasMax);
        allowed.insert(PropertyType::ReasonString);
        allowed.insert(PropertyType::SubIdAvail);
        allowed.insert(PropertyType::UserProperty);
        allowed.insert(PropertyType::WildcardSubAvail);
        allowed.insert(PropertyType::ShardSubAvail);
        allowed.insert(PropertyType::KeepAlive);
        allowed.insert(PropertyType::RespInfo);
        allowed.insert(PropertyType::ServerReference);
        allowed.insert(PropertyType::AuthMethod);
        allowed.insert(PropertyType::AuthData);

        allowed
    }

    pub fn reason(&self) -> Reason {
        self.reason
    }

    pub fn session_expiry(&self) -> Option<u32> {
        self.properties
            .get_property(PropertyType::SessionExpiryInterval)
            .and_then(|p| {
                if let Property::SessionExpiryInterval(interval) = p {
                    Some(*interval)
                } else {
                    None
                }
            })
    }

    pub fn set_session_expiry(&mut self, interval: u32) {
        if interval == 0 {
            self.properties
                .clear_property(PropertyType::SessionExpiryInterval);
            return;
        }
        self.properties
            .set_property(Property::SessionExpiryInterval(interval));
    }

    pub fn assigned_client_id(&self) -> Option<String> {
        self.properties
            .get_property(PropertyType::AssignedClientId)
            .and_then(|p| {
                if let Property::AssignedClientId(client_id) = p {
                    Some(client_id.clone())
                } else {
                    None
                }
            })
    }

    pub fn set_assigned_client_id(&mut self, client_id: Option<String>) {
        if let Some(client_id) = client_id {
            self.properties
                .set_property(Property::AssignedClientId(client_id));
        } else {
            self.properties
                .clear_property(PropertyType::AssignedClientId);
        }
    }

    pub fn server_keep_alive(&self) -> Option<u16> {
        self.properties
            .get_property(PropertyType::KeepAlive)
            .and_then(|p| {
                if let Property::KeepAlive(keep_alive) = p {
                    Some(*keep_alive)
                } else {
                    None
                }
            })
    }

    pub fn set_server_keep_alive(&mut self, keep_alive: u16) {
        if keep_alive == 0 {
            self.properties.clear_property(PropertyType::KeepAlive);
            return;
        }
        self.properties
            .set_property(Property::KeepAlive(keep_alive));
    }

    pub fn receive_max(&self) -> Option<u16> {
        self.properties
            .get_property(PropertyType::RecvMax)
            .and_then(|p| {
                if let Property::RecvMax(max) = p {
                    Some(*max)
                } else {
                    None
                }
            })
    }

    /// Sets the maximum number of QoS 1 and QoS 2 messages the session is willing to process.
    /// This is set as the CONNACK
    /// [Receive Maximum](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901049)
    /// property.The property is optional and if set to 0, the property is cleared as it is a
    /// protocol error to have a value of 0.
    ///
    /// # Arguments
    /// max - The maximum number of QoS 1 and QoS 2 messages the session is willing to process.
    pub fn set_receive_max(&mut self, max: u16) {
        if max == 0 {
            self.properties.clear_property(PropertyType::RecvMax);
            return;
        }
        self.properties.set_property(Property::RecvMax(max));
    }

    pub fn properties(&self) -> &PropertyBundle {
        &self.properties
    }

    pub fn properties_mut(&mut self) -> &mut PropertyBundle {
        &mut self.properties
    }
}

impl Default for ConnAck {
    fn default() -> Self {
        ConnAck {
            session_present: false,
            reason: Reason::Success,
            properties: PropertyBundle::new(ConnAck::allowed_properties()),
        }
    }
}

impl crate::Size for ConnAck {
    fn size(&self) -> u32 {
        // variable header is 3 bytes
        3 + self.property_size()
    }

    fn property_size(&self) -> u32 {
        self.properties.size()
    }

    /// Implementation of PacketSize. CONNACK packet does not have a payload.
    fn payload_size(&self) -> u32 {
        0
    }
}

impl Decode for ConnAck {
    fn decode(&mut self, src: &mut BytesMut) -> Result<(), MqttCodecError> {
        self.session_present = (0x01 & src.get_u8()) > 0;
        if let Ok(reason) = src.get_u8().try_into() {
            self.reason = reason;
        }
        self.properties.decode(src)?;
        Ok(())
    }
}

impl Encode for ConnAck {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
        let mut header = FixedHeader::new(PacketType::ConnAck);
        header.set_remaining(self.size());
        header.encode(dest)?;
        dest.put_u8(self.session_present as u8);
        dest.put_u8(self.reason as u8);
        // reserve capacity to avoid intermediate reallocation
        dest.reserve(self.property_size() as usize);
        self.properties.encode(dest)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::property::{Property, PropertyType};
    use crate::Encode;
    use bytes::BytesMut;

    /// Minimum length CONNACK return
    /// Byte 1 = packet type + flags
    /// Byte 2 = 1 byte variable byte integer remaining length
    /// Byte 3 = CONNACK flags
    /// Byte 4 = Reason
    /// Byte 5 = 1 byte variable byte integer property length
    const EXPECTED_MIN_CONNACK_LEN: usize = 5;

    #[test]
    fn test_complex_remaining() {
        const REASON: &str = "Malformed Packet";
        let auth_data = vec![0, 1, 2, 3, 4, 5];
        let expected_len = (REASON.len() + auth_data.len() + 11) as u32;
        let mut connack = ConnAck::default();
        connack.reason = Reason::MalformedPacket;
        let props = connack.properties_mut();
        props.set_property(Property::ReasonString(REASON.to_owned()));
        props.set_property(Property::AuthData(auth_data.clone()));
        props.set_property(Property::ShardSubAvail(false));
        assert_eq!(expected_len, connack.size());
    }

    #[test]
    fn test_encode_session_expiry_interval() {
        const EXPECTED_LEN: u32 = EXPECTED_MIN_CONNACK_LEN as u32 + 5;
        const EXPECTED_PROP_LEN: u32 = 5;
        let mut dest = BytesMut::new();
        let mut connack = ConnAck::default();
        let props = connack.properties_mut();
        props.set_property(Property::SessionExpiryInterval(257));
        test_property(
            connack,
            &mut dest,
            EXPECTED_LEN,
            EXPECTED_PROP_LEN,
            PropertyType::SessionExpiryInterval,
        );
        // 0x00000101 in bytes 6-9
        assert_eq!(1, dest[8]);
        assert_eq!(1, dest[9]);
    }

    #[test]
    fn test_decode_session_expiry_interval() {
        const EXPECTED_SESSION_EXPIRY: u32 = 0x00001010;
        let encoded = [
            PacketType::ConnAck as u8,
            0x08,
            0x00,
            0x00,
            0x05,
            PropertyType::SessionExpiryInterval as u8,
            0x00,
            0x00,
            0x10,
            0x10,
        ];
        let mut connack = ConnAck::default();
        let mut buf = BytesMut::from(&encoded[..]);
        buf.advance(2);
        let result = connack.decode(&mut buf);
        assert!(result.is_ok());
        assert!(connack
            .properties()
            .has_property(PropertyType::SessionExpiryInterval));
        if let Property::SessionExpiryInterval(interval) =
            connack.properties()[PropertyType::SessionExpiryInterval]
        {
            assert_eq!(EXPECTED_SESSION_EXPIRY, interval);
        }
    }

    #[test]
    fn test_encode_recv_max() {
        const EXPECTED_LEN: u32 = EXPECTED_MIN_CONNACK_LEN as u32 + 3;
        const EXPECTED_PROP_LEN: u32 = 3;
        let mut dest = BytesMut::new();
        let mut connack = ConnAck::default();
        let props = connack.properties_mut();
        props.set_property(Property::RecvMax(257));
        test_property(
            connack,
            &mut dest,
            EXPECTED_LEN,
            EXPECTED_PROP_LEN,
            PropertyType::RecvMax,
        );
        assert_eq!(1, dest[6]);
        assert_eq!(1, dest[7]);
    }

    #[test]
    fn test_decode_recv_max() {
        const EXPECTED_RECEIVE_MAX: u16 = 0x1234;
        let encoded = [
            PacketType::ConnAck as u8,
            0x06,
            0x00,
            0x00,
            0x03,
            PropertyType::RecvMax as u8,
            0x12,
            0x34,
        ];
        let mut connack = ConnAck::default();
        let mut buf = BytesMut::from(&encoded[..]);
        buf.advance(2);
        let result = connack.decode(&mut buf);
        assert!(result.is_ok(), "expected successful decode");
        assert!(
            connack.properties().has_property(PropertyType::RecvMax),
            "expected property to be set"
        );
        if let Property::RecvMax(max) = connack.properties()[PropertyType::RecvMax] {
            assert_eq!(EXPECTED_RECEIVE_MAX, max);
        }
    }

    fn test_property(
        connack: ConnAck,
        dest: &mut BytesMut,
        expected_len: u32,
        expected_prop_len: u32,
        property: PropertyType,
    ) {
        let result = connack.encode(dest);
        assert!(result.is_ok());
        assert_eq!(expected_len, dest.len() as u32);
        assert_eq!(expected_prop_len, connack.property_size());
        assert_eq!(expected_prop_len as u8, dest[4]);
        assert_eq!(property as u8, dest[5]);
    }
}
