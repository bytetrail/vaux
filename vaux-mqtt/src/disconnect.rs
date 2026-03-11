use crate::{codec, property::UserProperty, MqttCodecError, PropertyType, Reason};
use vaux_macro::{PropertyCodecSize, packet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Disconnect {
    fixed_header: codec::FixedHeader,
    pub reason: Reason,
    //#[codec(property_type = "PropertyType::SessionExpiryInterval")]
    pub session_expiry_interval: Option<u32>,
    //#[codec(property_type = "PropertyType::ReasonString")]
    pub reason_string: Option<String>,
   // #[codec(property_type = "PropertyType::ServerReference")]
    pub server_reference: Option<String>,
    //#[codec(property_type = "PropertyType::UserProperty")]
    pub user_properties: UserProperty,
}

impl Default for Disconnect {
    fn default() -> Self {
        Self {
            fixed_header: codec::FixedHeader::new(codec::PacketType::Disconnect),
            reason: Reason::NormalDisconnect,
            session_expiry_interval: None,
            reason_string: None,
            server_reference: None,
            user_properties: UserProperty::default(),
        }
    }
}

impl Disconnect {
    pub fn new(reason: Reason) -> Self {
        Self {
            fixed_header : codec::FixedHeader::new(codec::PacketType::Disconnect),
            reason,
            ..Default::default()
        }
    }
}



impl Disconnect {
    pub fn new_with_fixed_header(
        fixed_header: codec::FixedHeader,
    ) -> Result<Self, codec::MqttCodecError> {
        // if fixed_header.packet_type != codec::PacketType::Disconnect {
        //     return Err(
        //         MqttCodecError::new(
        //             ::alloc::__export::must_use({
        //                     ::alloc::fmt::format(
        //                         format_args!(
        //                             "Unsuppprted PacketType for {0}",
        //                             "#struct_name",
        //                         ),
        //                     )
        //                 })
        //                 .as_str(),
        //         ),
        //     );
        // }
        Ok(Self {
            fixed_header,
            ..Default::default()
        })
    }
}

impl codec::CodecSize for Disconnect {
    fn codec_size(&self) -> u32 {
        use codec::PropertyCodecSize;
        let mut total_size = 0;
        total_size += self.reason.codec_size();
        let property_size = self.property_size();
        total_size + property_size + codec::variable_byte_int_size(property_size)
    }
}

    impl codec::PropertyCodecSize for Disconnect {
        fn property_size(&self) -> u32 {
            use codec::CodecSize;
            let mut property_size = 0;
            if let Some(_) = &self.session_expiry_interval {
                property_size += 1 + 4;
            }
            if let Some(field_name) = &self.reason_string {
                let value_size = field_name.len() as u32 + 2;
                property_size += 1 + value_size;
            }
            if let Some(field_name) = &self.server_reference {
                let value_size = field_name.len() as u32 + 2;
                property_size += 1 + value_size;
            }
            property_size += self.user_properties.property_size();
            property_size
        }
    }

    impl codec::Encode for Disconnect {
        fn encode(&self, dest: &mut bytes::BytesMut) -> Result<(), MqttCodecError> {
            use bytes::{BufMut, BytesMut};
            use codec::{CodecSize, PropertyCodecSize};
            self.fixed_header.encode(dest)?;
            codec::encode_variable_byte_int(self.codec_size(), dest)?;
            self.reason.encode(dest)?;
            codec::encode_variable_byte_int(self.property_size(), dest)?;
            if let Some(v) = self.session_expiry_interval {
                dest.put_u8(PropertyType::SessionExpiryInterval as u8);
                dest.put_u32(v);
            }
            if let Some(v) = self.reason_string.as_ref() {
                dest.put_u8(PropertyType::ReasonString as u8);
                codec::encode_string(v, dest)?;
            }
            if let Some(v) = self.server_reference.as_ref() {
                dest.put_u8(PropertyType::ServerReference as u8);
                codec::encode_string(v, dest)?;
            }
            self.user_properties.encode(dest)?;
            Ok(())
        }
    }

    impl codec::Decode for Disconnect {
        fn decode(
            &mut self,
            src: &mut bytes::BytesMut,
        ) -> Result<usize, MqttCodecError> {
            use bytes::{BufMut, Buf, BytesMut};
            let mut bytes_read = 0_usize;
            let mut min_decode_len = 0usize;
            let required_remaining = if bytes_read < 0usize {
                0usize - bytes_read
            } else {
                0
            };
            if src.remaining() < required_remaining {
                return Err(
                    codec::MqttCodecError::new_with_kind(
                        format!(
                            "Insufficient data for decoding {}: expected at least {} bytes, got {}",
                            "Disconnect",
                            required_remaining,
                            src.remaining(),
                        ).as_str(),
                        // ::alloc::__export::must_use({
                        //         ::alloc::fmt::format(
                        //             format_args!(
                        //                 "Insufficient data for decoding {0}: expected at least {1} bytes, got {2}",
                        //                 "Disconnect",
                        //                 0usize,
                        //                 src.remaining(),
                        //             ),
                        //         )
                        //     })
                        //     .as_str(),
                        codec::ErrorKind::InsufficientData(
                            required_remaining,
                            src.remaining() as usize,
                        ),
                    ),
                );
            } else if src.remaining() == 0 && bytes_read == 0usize {
                return Ok(bytes_read);
            }
            bytes_read += self.reason.decode(src)?;
            let required_remaining = if bytes_read < 0usize {
                0usize - bytes_read
            } else {
                0
            };
            if src.remaining() < required_remaining {
                return Err(
                    codec::MqttCodecError::new_with_kind(
                        format!(
                            "Insufficient data for decoding {}: expected at least {} bytes, got {}",
                            "Disconnect",
                            0usize,
                            src.remaining(),
                        ).as_str(),
                        codec::ErrorKind::InsufficientData(
                            required_remaining,
                            src.remaining() as usize,
                        ),
                    ),
                );
            } else if src.remaining() == 0 && bytes_read == 0usize {
                return Ok(bytes_read);
            }
            let (property_length, var_bytes_read) = codec::decode_variable_byte_int(
                src,
            )?;
            let property_length = property_length as usize;
            bytes_read += var_bytes_read;
            let mut property_bytes_read = 0_usize;
            while property_bytes_read < property_length {
                let property_type = src.get_u8().try_into()?;
                property_bytes_read += 1;
                match property_type {
                    PropertyType::SessionExpiryInterval => {
                        let mut value = u32::default();
                        property_bytes_read += value.decode(src)?;
                        self.session_expiry_interval = Some(value);
                    }
                    PropertyType::ReasonString => {
                        let mut value = String::default();
                        property_bytes_read += value.decode(src)?;
                        self.reason_string = Some(value);
                    }
                    PropertyType::ServerReference => {
                        let mut value = String::default();
                        property_bytes_read += value.decode(src)?;
                        self.server_reference = Some(value);
                    }
                    PropertyType::UserProperty => {
                        property_bytes_read += self.user_properties.decode(src)?;
                    }
                    _ => {
                        return Err(
                            codec::MqttCodecError::new_with_kind(
                                format!(
                                    "MQTT v5 property type {0:?} is not supported",
                                    property_type,
                                ).as_str(),
                                codec::ErrorKind::UnsupportedProperty(property_type as u8),
                            ),
                        );
                    }
                }
            }
            bytes_read += property_bytes_read;

            bytes_read += property_bytes_read;
            Ok(bytes_read)
        }
    }

