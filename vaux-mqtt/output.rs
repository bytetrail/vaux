#![feature(prelude_import)]
#[macro_use]
extern crate std;
#[prelude_import]
use std::prelude::rust_2021::*;
pub mod codec {
    pub mod fixed {
        use bytes::{Buf, BufMut, BytesMut};
        use crate::{
            codec::{
                Decode, Encode, ErrorKind, PACKET_RESERVED_BIT1, PACKET_RESERVED_NONE,
            },
            MqttCodecError, PacketType, QoSLevel,
        };
        const QOS_MASK: u8 = 0b_0000_0110;
        const RETAIN_MASK: u8 = 0b_0000_0001;
        const DUP_MASK: u8 = 0b_0000_1000;
        pub struct FixedHeader {
            pub packet_type: PacketType,
            flags: u8,
        }
        #[automatically_derived]
        impl ::core::default::Default for FixedHeader {
            #[inline]
            fn default() -> FixedHeader {
                FixedHeader {
                    packet_type: ::core::default::Default::default(),
                    flags: ::core::default::Default::default(),
                }
            }
        }
        #[automatically_derived]
        impl ::core::fmt::Debug for FixedHeader {
            #[inline]
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                ::core::fmt::Formatter::debug_struct_field2_finish(
                    f,
                    "FixedHeader",
                    "packet_type",
                    &self.packet_type,
                    "flags",
                    &&self.flags,
                )
            }
        }
        #[automatically_derived]
        impl ::core::clone::Clone for FixedHeader {
            #[inline]
            fn clone(&self) -> FixedHeader {
                FixedHeader {
                    packet_type: ::core::clone::Clone::clone(&self.packet_type),
                    flags: ::core::clone::Clone::clone(&self.flags),
                }
            }
        }
        #[automatically_derived]
        impl ::core::cmp::Eq for FixedHeader {
            #[inline]
            #[doc(hidden)]
            #[coverage(off)]
            fn assert_receiver_is_total_eq(&self) -> () {
                let _: ::core::cmp::AssertParamIsEq<PacketType>;
                let _: ::core::cmp::AssertParamIsEq<u8>;
            }
        }
        #[automatically_derived]
        impl ::core::marker::StructuralPartialEq for FixedHeader {}
        #[automatically_derived]
        impl ::core::cmp::PartialEq for FixedHeader {
            #[inline]
            fn eq(&self, other: &FixedHeader) -> bool {
                self.flags == other.flags && self.packet_type == other.packet_type
            }
        }
        impl FixedHeader {
            pub fn new(packet_type: PacketType) -> Self {
                match packet_type {
                    PacketType::PubRel
                    | PacketType::Subscribe
                    | PacketType::Unsubscribe => {
                        FixedHeader {
                            packet_type,
                            flags: PACKET_RESERVED_BIT1,
                        }
                    }
                    _ => {
                        FixedHeader {
                            packet_type,
                            flags: PACKET_RESERVED_NONE,
                        }
                    }
                }
            }
            pub fn packet_type(&self) -> PacketType {
                self.packet_type
            }
            pub fn dup(&self) -> bool {
                (self.flags & DUP_MASK) != 0
            }
            pub fn set_dup(&mut self, dup: bool) {
                self.flags = self.flags & !DUP_MASK | ((dup as u8) << 3);
            }
            pub fn retain(&self) -> bool {
                (self.flags & RETAIN_MASK) != 0
            }
            pub fn set_retain(&mut self, retain: bool) {
                self.flags = self.flags & !RETAIN_MASK | retain as u8;
            }
            pub fn qos(&self) -> QoSLevel {
                ((self.flags & QOS_MASK) >> 1).try_into().unwrap()
            }
            pub fn set_qos(&mut self, qos: QoSLevel) {
                self.flags = self.flags & !QOS_MASK | ((qos as u8) << 1);
            }
            pub fn flags(&self) -> u8 {
                self.flags
            }
            pub fn set_flags(&mut self, flags: u8) -> Result<(), MqttCodecError> {
                if flags & QOS_MASK == QOS_MASK {
                    return Err(MqttCodecError {
                        reason: "unsupported QOS level".to_string(),
                        kind: ErrorKind::UnsupportedQosLevel,
                    });
                }
                self.flags = flags;
                Ok(())
            }
            pub fn clear_flags(&mut self) {
                self.flags = 0;
            }
        }
        impl Encode for FixedHeader {
            fn encode(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
                let first_byte = (self.packet_type as u8) | (self.flags & 0x0f);
                dest.put_u8(first_byte);
                Ok(())
            }
        }
        impl Decode for FixedHeader {
            fn decode(&mut self, src: &mut BytesMut) -> Result<u32, MqttCodecError> {
                if src.remaining() < 2 {
                    return Err(
                        MqttCodecError::new_with_kind(
                            "Insufficient data",
                            ErrorKind::InsufficientData(2, src.remaining()),
                        ),
                    );
                }
                let first_byte = src.get_u8();
                let flags = first_byte & 0x0f;
                self.packet_type = match PacketType::try_from(first_byte & 0xF0) {
                    Ok(pt) => pt,
                    Err(_) => {
                        return Err(MqttCodecError {
                            reason: "invalid packet type".to_string(),
                            kind: ErrorKind::MalformedPacket,
                        });
                    }
                };
                match self.packet_type {
                    PacketType::Connect
                    | PacketType::PubRel
                    | PacketType::PubAck
                    | PacketType::Subscribe
                    | PacketType::Unsubscribe
                    | PacketType::ConnAck
                    | PacketType::PubRec
                    | PacketType::PubComp
                    | PacketType::SubAck
                    | PacketType::UnsubAck
                    | PacketType::PingReq
                    | PacketType::PingResp
                    | PacketType::Disconnect
                    | PacketType::Auth => {
                        if flags != PACKET_RESERVED_NONE {
                            return Err(
                                MqttCodecError::new(
                                    ::alloc::__export::must_use({
                                            ::alloc::fmt::format(
                                                format_args!(
                                                    "invalid flags for {0}: {1}",
                                                    self.packet_type,
                                                    flags,
                                                ),
                                            )
                                        })
                                        .as_str(),
                                ),
                            );
                        }
                    }
                    _ => {}
                }
                self.flags = flags;
                Ok(1)
            }
        }
    }
    pub use fixed::FixedHeader;
    use vaux_macro::packet;
    use crate::{
        ConnAck, Disconnect, Subscribe, connect::Connect, publish::Publish,
        pubresp::{PubAck, PubComp, PubRec, PubRel},
        subscribe::SubAck, unsubscribe::{UnsubAck, Unsubscribe},
    };
    use bytes::{Buf, BufMut, BytesMut};
    use std::fmt::{Display, Formatter};
    pub(crate) const PACKET_RESERVED_NONE: u8 = 0x00;
    pub(crate) const PACKET_RESERVED_BIT1: u8 = 0x02;
    pub const MIN_VARIABLE_BYTE_INT: u32 = 0;
    pub const MAX_VARIABLE_BYTE_INT: u32 = 268_435_455;
    pub trait PropertyCodecSize {
        fn property_size(&self) -> u32;
    }
    pub trait CodecSize {
        fn codec_size(&self) -> u32;
    }
    pub trait Encode {
        fn encode(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError>;
    }
    pub trait Decode {
        fn decode(&mut self, src: &mut BytesMut) -> Result<u32, MqttCodecError>;
    }
    /// MQTT Control Packet Type
    /// #[repr(u8)]
    pub enum PacketType {
        #[default]
        Connect = 0x10,
        ConnAck = 0x20,
        Publish = 0x30,
        PubAck = 0x40,
        PubRec = 0x50,
        PubRel = 0x60,
        PubComp = 0x70,
        Subscribe = 0x80,
        SubAck = 0x90,
        Unsubscribe = 0xa0,
        UnsubAck = 0xb0,
        PingReq = 0xc0,
        PingResp = 0xd0,
        Disconnect = 0xe0,
        Auth = 0xf0,
    }
    #[automatically_derived]
    impl ::core::default::Default for PacketType {
        #[inline]
        fn default() -> PacketType {
            Self::Connect
        }
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for PacketType {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::write_str(
                f,
                match self {
                    PacketType::Connect => "Connect",
                    PacketType::ConnAck => "ConnAck",
                    PacketType::Publish => "Publish",
                    PacketType::PubAck => "PubAck",
                    PacketType::PubRec => "PubRec",
                    PacketType::PubRel => "PubRel",
                    PacketType::PubComp => "PubComp",
                    PacketType::Subscribe => "Subscribe",
                    PacketType::SubAck => "SubAck",
                    PacketType::Unsubscribe => "Unsubscribe",
                    PacketType::UnsubAck => "UnsubAck",
                    PacketType::PingReq => "PingReq",
                    PacketType::PingResp => "PingResp",
                    PacketType::Disconnect => "Disconnect",
                    PacketType::Auth => "Auth",
                },
            )
        }
    }
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for PacketType {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for PacketType {
        #[inline]
        fn eq(&self, other: &PacketType) -> bool {
            let __self_discr = ::core::intrinsics::discriminant_value(self);
            let __arg1_discr = ::core::intrinsics::discriminant_value(other);
            __self_discr == __arg1_discr
        }
    }
    #[automatically_derived]
    impl ::core::cmp::Eq for PacketType {
        #[inline]
        #[doc(hidden)]
        #[coverage(off)]
        fn assert_receiver_is_total_eq(&self) -> () {}
    }
    #[automatically_derived]
    impl ::core::marker::Copy for PacketType {}
    #[automatically_derived]
    impl ::core::clone::Clone for PacketType {
        #[inline]
        fn clone(&self) -> PacketType {
            *self
        }
    }
    #[automatically_derived]
    impl ::core::hash::Hash for PacketType {
        #[inline]
        fn hash<__H: ::core::hash::Hasher>(&self, state: &mut __H) -> () {
            let __self_discr = ::core::intrinsics::discriminant_value(self);
            ::core::hash::Hash::hash(&__self_discr, state)
        }
    }
    impl From<u8> for PacketType {
        fn from(val: u8) -> Self {
            match val & 0xf0 {
                0x10 => PacketType::Connect,
                0x20 => PacketType::ConnAck,
                0x30 => PacketType::Publish,
                0x40 => PacketType::PubAck,
                0x50 => PacketType::PubRec,
                0x60 => PacketType::PubRel,
                0x70 => PacketType::PubComp,
                0x80 => PacketType::Subscribe,
                0x90 => PacketType::SubAck,
                0xa0 => PacketType::Unsubscribe,
                0xb0 => PacketType::UnsubAck,
                0xc0 => PacketType::PingReq,
                0xd0 => PacketType::PingResp,
                0xe0 => PacketType::Disconnect,
                _ => PacketType::Auth,
            }
        }
    }
    impl Display for PacketType {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(
                format_args!(
                    "{0}",
                    ::alloc::__export::must_use({
                            ::alloc::fmt::format(format_args!("{0:?}", &self))
                        })
                        .as_str()
                        .to_uppercase(),
                ),
            )
        }
    }
    /// Reason code is a 1 byte unsigned value that indicates the result of a control
    /// packet request. For more information on reason codes see the MQTT Specification,
    /// <https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901031>
    #[repr(u8)]
    pub enum Reason {
        #[default]
        Success,
        GrantedQoS1,
        GrantedQoS2,
        DisconnectWillMsg = 0x04,
        NoSubscribers = 0x10,
        NoSubscriptionExisted,
        ContinueAuth = 0x18,
        Reauthenticate,
        UnspecifiedErr = 0x80,
        MalformedPacket,
        ProtocolErr,
        ImplementationErr,
        UnsupportedProtocolVersion,
        InvalidClientId,
        AuthenticationErr,
        NotAuthorized,
        ServerUnavailable,
        ServerBusy,
        Banned,
        ServerShutdown,
        AuthMethodErr,
        KeepAliveTimeout,
        SessionTakeOver,
        InvalidTopicFilter,
        InvalidTopicName,
        PacketIdInUse,
        PacketIdNotFound,
        ReceiveMaxExceeded,
        InvalidTopicAlias,
        PacketTooLarge,
        MessageRate,
        QuotaExceeded,
        AdminAction,
        PayloadFormatErr,
        RetainUnsupported,
        QoSUnsupported,
        UseDiffServer,
        ServerMoved,
        SharedSubUnsupported,
        ConnRateExceeded,
        MaxConnectTime,
        SubIdUnsupported,
        WildcardSubUnsupported,
    }
    #[automatically_derived]
    impl ::core::default::Default for Reason {
        #[inline]
        fn default() -> Reason {
            Self::Success
        }
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for Reason {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::write_str(
                f,
                match self {
                    Reason::Success => "Success",
                    Reason::GrantedQoS1 => "GrantedQoS1",
                    Reason::GrantedQoS2 => "GrantedQoS2",
                    Reason::DisconnectWillMsg => "DisconnectWillMsg",
                    Reason::NoSubscribers => "NoSubscribers",
                    Reason::NoSubscriptionExisted => "NoSubscriptionExisted",
                    Reason::ContinueAuth => "ContinueAuth",
                    Reason::Reauthenticate => "Reauthenticate",
                    Reason::UnspecifiedErr => "UnspecifiedErr",
                    Reason::MalformedPacket => "MalformedPacket",
                    Reason::ProtocolErr => "ProtocolErr",
                    Reason::ImplementationErr => "ImplementationErr",
                    Reason::UnsupportedProtocolVersion => "UnsupportedProtocolVersion",
                    Reason::InvalidClientId => "InvalidClientId",
                    Reason::AuthenticationErr => "AuthenticationErr",
                    Reason::NotAuthorized => "NotAuthorized",
                    Reason::ServerUnavailable => "ServerUnavailable",
                    Reason::ServerBusy => "ServerBusy",
                    Reason::Banned => "Banned",
                    Reason::ServerShutdown => "ServerShutdown",
                    Reason::AuthMethodErr => "AuthMethodErr",
                    Reason::KeepAliveTimeout => "KeepAliveTimeout",
                    Reason::SessionTakeOver => "SessionTakeOver",
                    Reason::InvalidTopicFilter => "InvalidTopicFilter",
                    Reason::InvalidTopicName => "InvalidTopicName",
                    Reason::PacketIdInUse => "PacketIdInUse",
                    Reason::PacketIdNotFound => "PacketIdNotFound",
                    Reason::ReceiveMaxExceeded => "ReceiveMaxExceeded",
                    Reason::InvalidTopicAlias => "InvalidTopicAlias",
                    Reason::PacketTooLarge => "PacketTooLarge",
                    Reason::MessageRate => "MessageRate",
                    Reason::QuotaExceeded => "QuotaExceeded",
                    Reason::AdminAction => "AdminAction",
                    Reason::PayloadFormatErr => "PayloadFormatErr",
                    Reason::RetainUnsupported => "RetainUnsupported",
                    Reason::QoSUnsupported => "QoSUnsupported",
                    Reason::UseDiffServer => "UseDiffServer",
                    Reason::ServerMoved => "ServerMoved",
                    Reason::SharedSubUnsupported => "SharedSubUnsupported",
                    Reason::ConnRateExceeded => "ConnRateExceeded",
                    Reason::MaxConnectTime => "MaxConnectTime",
                    Reason::SubIdUnsupported => "SubIdUnsupported",
                    Reason::WildcardSubUnsupported => "WildcardSubUnsupported",
                },
            )
        }
    }
    #[automatically_derived]
    impl ::core::cmp::Eq for Reason {
        #[inline]
        #[doc(hidden)]
        #[coverage(off)]
        fn assert_receiver_is_total_eq(&self) -> () {}
    }
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for Reason {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for Reason {
        #[inline]
        fn eq(&self, other: &Reason) -> bool {
            let __self_discr = ::core::intrinsics::discriminant_value(self);
            let __arg1_discr = ::core::intrinsics::discriminant_value(other);
            __self_discr == __arg1_discr
        }
    }
    #[automatically_derived]
    impl ::core::marker::Copy for Reason {}
    #[automatically_derived]
    impl ::core::clone::Clone for Reason {
        #[inline]
        fn clone(&self) -> Reason {
            *self
        }
    }
    impl Display for Reason {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{0:?}", self))
        }
    }
    #[allow(non_upper_case_globals)]
    impl Reason {
        pub const NormalDisconnect: Reason = Reason::Success;
        pub const GrantedQoS0: Reason = Reason::Success;
    }
    impl TryFrom<u8> for Reason {
        type Error = MqttCodecError;
        fn try_from(value: u8) -> Result<Self, Self::Error> {
            match value {
                0x00 => Ok(Reason::Success),
                0x01 => Ok(Reason::GrantedQoS1),
                0x02 => Ok(Reason::GrantedQoS2),
                0x04 => Ok(Reason::DisconnectWillMsg),
                0x10 => Ok(Reason::NoSubscribers),
                0x11 => Ok(Reason::NoSubscriptionExisted),
                0x18 => Ok(Reason::ContinueAuth),
                0x19 => Ok(Reason::Reauthenticate),
                0x80 => Ok(Reason::UnspecifiedErr),
                0x81 => Ok(Reason::MalformedPacket),
                0x82 => Ok(Reason::ProtocolErr),
                0x83 => Ok(Reason::ImplementationErr),
                0x84 => Ok(Reason::UnsupportedProtocolVersion),
                0x85 => Ok(Reason::InvalidClientId),
                0x86 => Ok(Reason::AuthenticationErr),
                0x87 => Ok(Reason::NotAuthorized),
                0x88 => Ok(Reason::ServerUnavailable),
                0x89 => Ok(Reason::ServerBusy),
                0x8a => Ok(Reason::Banned),
                0x8b => Ok(Reason::ServerShutdown),
                0x8c => Ok(Reason::AuthMethodErr),
                0x8d => Ok(Reason::KeepAliveTimeout),
                0x8e => Ok(Reason::SessionTakeOver),
                0x8f => Ok(Reason::InvalidTopicFilter),
                0x90 => Ok(Reason::InvalidTopicName),
                0x91 => Ok(Reason::PacketIdInUse),
                0x92 => Ok(Reason::PacketIdNotFound),
                0x93 => Ok(Reason::ReceiveMaxExceeded),
                0x94 => Ok(Reason::InvalidTopicAlias),
                0x95 => Ok(Reason::PacketTooLarge),
                0x96 => Ok(Reason::MessageRate),
                0x97 => Ok(Reason::QuotaExceeded),
                0x98 => Ok(Reason::AdminAction),
                0x99 => Ok(Reason::PayloadFormatErr),
                0x9a => Ok(Reason::RetainUnsupported),
                0x9b => Ok(Reason::QoSUnsupported),
                0x9c => Ok(Reason::UseDiffServer),
                0x9d => Ok(Reason::ServerMoved),
                0x9e => Ok(Reason::SharedSubUnsupported),
                0x9f => Ok(Reason::ConnRateExceeded),
                0xa0 => Ok(Reason::MaxConnectTime),
                0xa1 => Ok(Reason::SubIdUnsupported),
                0xa2 => Ok(Reason::WildcardSubUnsupported),
                value => {
                    Err(
                        MqttCodecError::new(
                            &::alloc::__export::must_use({
                                ::alloc::fmt::format(
                                    format_args!(" Unsupported reason code: {0}", value),
                                )
                            }),
                        ),
                    )
                }
            }
        }
    }
    impl CodecSize for Reason {
        fn codec_size(&self) -> u32 {
            1
        }
    }
    impl Encode for Reason {
        fn encode(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
            dest.put_u8(*self as u8);
            Ok(())
        }
    }
    impl Decode for Reason {
        fn decode(&mut self, src: &mut BytesMut) -> Result<u32, MqttCodecError> {
            let val = src.get_u8();
            *self = val.try_into()?;
            Ok(1)
        }
    }
    #[allow(clippy::enum_variant_names)]
    #[repr(u8)]
    pub enum QoSLevel {
        #[default]
        AtMostOnce = 0,
        AtLeastOnce = 1,
        ExactlyOnce = 2,
    }
    #[automatically_derived]
    #[allow(clippy::enum_variant_names)]
    impl ::core::default::Default for QoSLevel {
        #[inline]
        fn default() -> QoSLevel {
            Self::AtMostOnce
        }
    }
    #[automatically_derived]
    #[allow(clippy::enum_variant_names)]
    impl ::core::fmt::Debug for QoSLevel {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::write_str(
                f,
                match self {
                    QoSLevel::AtMostOnce => "AtMostOnce",
                    QoSLevel::AtLeastOnce => "AtLeastOnce",
                    QoSLevel::ExactlyOnce => "ExactlyOnce",
                },
            )
        }
    }
    #[automatically_derived]
    #[allow(clippy::enum_variant_names)]
    impl ::core::marker::StructuralPartialEq for QoSLevel {}
    #[automatically_derived]
    #[allow(clippy::enum_variant_names)]
    impl ::core::cmp::PartialEq for QoSLevel {
        #[inline]
        fn eq(&self, other: &QoSLevel) -> bool {
            let __self_discr = ::core::intrinsics::discriminant_value(self);
            let __arg1_discr = ::core::intrinsics::discriminant_value(other);
            __self_discr == __arg1_discr
        }
    }
    #[automatically_derived]
    #[allow(clippy::enum_variant_names)]
    impl ::core::cmp::Eq for QoSLevel {
        #[inline]
        #[doc(hidden)]
        #[coverage(off)]
        fn assert_receiver_is_total_eq(&self) -> () {}
    }
    #[automatically_derived]
    #[allow(clippy::enum_variant_names)]
    impl ::core::marker::Copy for QoSLevel {}
    #[automatically_derived]
    #[allow(clippy::enum_variant_names)]
    impl ::core::clone::Clone for QoSLevel {
        #[inline]
        fn clone(&self) -> QoSLevel {
            *self
        }
    }
    #[automatically_derived]
    #[allow(clippy::enum_variant_names)]
    impl ::core::hash::Hash for QoSLevel {
        #[inline]
        fn hash<__H: ::core::hash::Hasher>(&self, state: &mut __H) -> () {
            let __self_discr = ::core::intrinsics::discriminant_value(self);
            ::core::hash::Hash::hash(&__self_discr, state)
        }
    }
    impl TryFrom<u8> for QoSLevel {
        type Error = MqttCodecError;
        fn try_from(value: u8) -> Result<Self, Self::Error> {
            match value {
                0x00 => Ok(QoSLevel::AtMostOnce),
                0x01 => Ok(QoSLevel::AtLeastOnce),
                0x02 => Ok(QoSLevel::ExactlyOnce),
                value => {
                    Err(
                        MqttCodecError::new(
                            &::alloc::__export::must_use({
                                ::alloc::fmt::format(
                                    format_args!("{0} is not a value QoSLevel", value),
                                )
                            }),
                        ),
                    )
                }
            }
        }
    }
    impl Encode for QoSLevel {
        fn encode(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
            dest.put_u8(*self as u8);
            Ok(())
        }
    }
    impl Decode for QoSLevel {
        fn decode(&mut self, src: &mut BytesMut) -> Result<u32, MqttCodecError> {
            let val = src.get_u8();
            *self = val.try_into()?;
            Ok(1)
        }
    }
    impl CodecSize for QoSLevel {
        fn codec_size(&self) -> u32 {
            1
        }
    }
    impl PropertyCodecSize for &QoSLevel {
        fn property_size(&self) -> u32 {
            2
        }
    }
    impl CodecSize for &String {
        fn codec_size(&self) -> u32 {
            2 + self.len() as u32
        }
    }
    impl Encode for &String {
        fn encode(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
            encode_string(self, dest)
        }
    }
    pub enum Packet {
        PingRequest(crate::PingReq),
        PingResponse(crate::PingResp),
        Connect(Box<Connect>),
        ConnAck(ConnAck),
        Publish(Publish),
        PubAck(PubAck),
        PubComp(PubComp),
        PubRec(PubRec),
        PubRel(PubRel),
        Disconnect(Disconnect),
        Subscribe(Subscribe),
        SubAck(SubAck),
        Unsubscribe(Unsubscribe),
        UnsubAck(UnsubAck),
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for Packet {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            match self {
                Packet::PingRequest(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(
                        f,
                        "PingRequest",
                        &__self_0,
                    )
                }
                Packet::PingResponse(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(
                        f,
                        "PingResponse",
                        &__self_0,
                    )
                }
                Packet::Connect(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(
                        f,
                        "Connect",
                        &__self_0,
                    )
                }
                Packet::ConnAck(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(
                        f,
                        "ConnAck",
                        &__self_0,
                    )
                }
                Packet::Publish(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(
                        f,
                        "Publish",
                        &__self_0,
                    )
                }
                Packet::PubAck(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(
                        f,
                        "PubAck",
                        &__self_0,
                    )
                }
                Packet::PubComp(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(
                        f,
                        "PubComp",
                        &__self_0,
                    )
                }
                Packet::PubRec(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(
                        f,
                        "PubRec",
                        &__self_0,
                    )
                }
                Packet::PubRel(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(
                        f,
                        "PubRel",
                        &__self_0,
                    )
                }
                Packet::Disconnect(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(
                        f,
                        "Disconnect",
                        &__self_0,
                    )
                }
                Packet::Subscribe(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(
                        f,
                        "Subscribe",
                        &__self_0,
                    )
                }
                Packet::SubAck(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(
                        f,
                        "SubAck",
                        &__self_0,
                    )
                }
                Packet::Unsubscribe(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(
                        f,
                        "Unsubscribe",
                        &__self_0,
                    )
                }
                Packet::UnsubAck(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(
                        f,
                        "UnsubAck",
                        &__self_0,
                    )
                }
            }
        }
    }
    #[automatically_derived]
    impl ::core::clone::Clone for Packet {
        #[inline]
        fn clone(&self) -> Packet {
            match self {
                Packet::PingRequest(__self_0) => {
                    Packet::PingRequest(::core::clone::Clone::clone(__self_0))
                }
                Packet::PingResponse(__self_0) => {
                    Packet::PingResponse(::core::clone::Clone::clone(__self_0))
                }
                Packet::Connect(__self_0) => {
                    Packet::Connect(::core::clone::Clone::clone(__self_0))
                }
                Packet::ConnAck(__self_0) => {
                    Packet::ConnAck(::core::clone::Clone::clone(__self_0))
                }
                Packet::Publish(__self_0) => {
                    Packet::Publish(::core::clone::Clone::clone(__self_0))
                }
                Packet::PubAck(__self_0) => {
                    Packet::PubAck(::core::clone::Clone::clone(__self_0))
                }
                Packet::PubComp(__self_0) => {
                    Packet::PubComp(::core::clone::Clone::clone(__self_0))
                }
                Packet::PubRec(__self_0) => {
                    Packet::PubRec(::core::clone::Clone::clone(__self_0))
                }
                Packet::PubRel(__self_0) => {
                    Packet::PubRel(::core::clone::Clone::clone(__self_0))
                }
                Packet::Disconnect(__self_0) => {
                    Packet::Disconnect(::core::clone::Clone::clone(__self_0))
                }
                Packet::Subscribe(__self_0) => {
                    Packet::Subscribe(::core::clone::Clone::clone(__self_0))
                }
                Packet::SubAck(__self_0) => {
                    Packet::SubAck(::core::clone::Clone::clone(__self_0))
                }
                Packet::Unsubscribe(__self_0) => {
                    Packet::Unsubscribe(::core::clone::Clone::clone(__self_0))
                }
                Packet::UnsubAck(__self_0) => {
                    Packet::UnsubAck(::core::clone::Clone::clone(__self_0))
                }
            }
        }
    }
    #[automatically_derived]
    impl ::core::cmp::Eq for Packet {
        #[inline]
        #[doc(hidden)]
        #[coverage(off)]
        fn assert_receiver_is_total_eq(&self) -> () {
            let _: ::core::cmp::AssertParamIsEq<crate::PingReq>;
            let _: ::core::cmp::AssertParamIsEq<crate::PingResp>;
            let _: ::core::cmp::AssertParamIsEq<Box<Connect>>;
            let _: ::core::cmp::AssertParamIsEq<ConnAck>;
            let _: ::core::cmp::AssertParamIsEq<Publish>;
            let _: ::core::cmp::AssertParamIsEq<PubAck>;
            let _: ::core::cmp::AssertParamIsEq<PubComp>;
            let _: ::core::cmp::AssertParamIsEq<PubRec>;
            let _: ::core::cmp::AssertParamIsEq<PubRel>;
            let _: ::core::cmp::AssertParamIsEq<Disconnect>;
            let _: ::core::cmp::AssertParamIsEq<Subscribe>;
            let _: ::core::cmp::AssertParamIsEq<SubAck>;
            let _: ::core::cmp::AssertParamIsEq<Unsubscribe>;
            let _: ::core::cmp::AssertParamIsEq<UnsubAck>;
        }
    }
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for Packet {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for Packet {
        #[inline]
        fn eq(&self, other: &Packet) -> bool {
            let __self_discr = ::core::intrinsics::discriminant_value(self);
            let __arg1_discr = ::core::intrinsics::discriminant_value(other);
            __self_discr == __arg1_discr
                && match (self, other) {
                    (Packet::PingRequest(__self_0), Packet::PingRequest(__arg1_0)) => {
                        __self_0 == __arg1_0
                    }
                    (Packet::PingResponse(__self_0), Packet::PingResponse(__arg1_0)) => {
                        __self_0 == __arg1_0
                    }
                    (Packet::Connect(__self_0), Packet::Connect(__arg1_0)) => {
                        __self_0 == __arg1_0
                    }
                    (Packet::ConnAck(__self_0), Packet::ConnAck(__arg1_0)) => {
                        __self_0 == __arg1_0
                    }
                    (Packet::Publish(__self_0), Packet::Publish(__arg1_0)) => {
                        __self_0 == __arg1_0
                    }
                    (Packet::PubAck(__self_0), Packet::PubAck(__arg1_0)) => {
                        __self_0 == __arg1_0
                    }
                    (Packet::PubComp(__self_0), Packet::PubComp(__arg1_0)) => {
                        __self_0 == __arg1_0
                    }
                    (Packet::PubRec(__self_0), Packet::PubRec(__arg1_0)) => {
                        __self_0 == __arg1_0
                    }
                    (Packet::PubRel(__self_0), Packet::PubRel(__arg1_0)) => {
                        __self_0 == __arg1_0
                    }
                    (Packet::Disconnect(__self_0), Packet::Disconnect(__arg1_0)) => {
                        __self_0 == __arg1_0
                    }
                    (Packet::Subscribe(__self_0), Packet::Subscribe(__arg1_0)) => {
                        __self_0 == __arg1_0
                    }
                    (Packet::SubAck(__self_0), Packet::SubAck(__arg1_0)) => {
                        __self_0 == __arg1_0
                    }
                    (Packet::Unsubscribe(__self_0), Packet::Unsubscribe(__arg1_0)) => {
                        __self_0 == __arg1_0
                    }
                    (Packet::UnsubAck(__self_0), Packet::UnsubAck(__arg1_0)) => {
                        __self_0 == __arg1_0
                    }
                    _ => unsafe { ::core::intrinsics::unreachable() }
                }
        }
    }
    impl Encode for Packet {
        fn encode(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
            match self {
                Packet::PingRequest(pingreq) => pingreq.encode(dest),
                Packet::PingResponse(pingresp) => pingresp.encode(dest),
                Packet::Connect(connect) => connect.encode(dest),
                Packet::ConnAck(connack) => connack.encode(dest),
                Packet::Publish(publish) => publish.encode(dest),
                Packet::PubAck(puback) => puback.encode(dest),
                Packet::PubComp(pubcomp) => pubcomp.encode(dest),
                Packet::PubRec(pubrec) => pubrec.encode(dest),
                Packet::PubRel(pubrel) => pubrel.encode(dest),
                Packet::Disconnect(disconnect) => disconnect.encode(dest),
                Packet::Subscribe(subscribe) => subscribe.encode(dest),
                Packet::SubAck(suback) => suback.encode(dest),
                Packet::Unsubscribe(unsubscribe) => unsubscribe.encode(dest),
                Packet::UnsubAck(unsuback) => unsuback.encode(dest),
            }
        }
    }
    impl From<&Packet> for PacketType {
        fn from(p: &Packet) -> Self {
            match p {
                Packet::PingRequest(_) => PacketType::PingReq,
                Packet::PingResponse(_) => PacketType::PingResp,
                Packet::Connect(_) => PacketType::Connect,
                Packet::ConnAck(_) => PacketType::ConnAck,
                Packet::Publish(_) => PacketType::Publish,
                Packet::PubAck(_) => PacketType::PubAck,
                Packet::PubComp(_) => PacketType::PubComp,
                Packet::PubRec(_) => PacketType::PubRec,
                Packet::PubRel(_) => PacketType::PubRel,
                Packet::Disconnect(_) => PacketType::Disconnect,
                Packet::Subscribe(_) => PacketType::Subscribe,
                Packet::SubAck(_) => PacketType::SubAck,
                Packet::Unsubscribe(_) => PacketType::Unsubscribe,
                Packet::UnsubAck(_) => PacketType::UnsubAck,
            }
        }
    }
    pub enum ErrorKind {
        InsufficientData(usize, usize),
        #[default]
        MalformedPacket,
        InvalidPacket,
        UnsupportedQosLevel,
        UnsupportedResponseType,
        UnsupportedReason(u8),
        UnsupportedProperty(u8),
        InvalidUTF8,
        InvalidPacketIdentifier,
    }
    #[automatically_derived]
    impl ::core::default::Default for ErrorKind {
        #[inline]
        fn default() -> ErrorKind {
            Self::MalformedPacket
        }
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for ErrorKind {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            match self {
                ErrorKind::InsufficientData(__self_0, __self_1) => {
                    ::core::fmt::Formatter::debug_tuple_field2_finish(
                        f,
                        "InsufficientData",
                        __self_0,
                        &__self_1,
                    )
                }
                ErrorKind::MalformedPacket => {
                    ::core::fmt::Formatter::write_str(f, "MalformedPacket")
                }
                ErrorKind::InvalidPacket => {
                    ::core::fmt::Formatter::write_str(f, "InvalidPacket")
                }
                ErrorKind::UnsupportedQosLevel => {
                    ::core::fmt::Formatter::write_str(f, "UnsupportedQosLevel")
                }
                ErrorKind::UnsupportedResponseType => {
                    ::core::fmt::Formatter::write_str(f, "UnsupportedResponseType")
                }
                ErrorKind::UnsupportedReason(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(
                        f,
                        "UnsupportedReason",
                        &__self_0,
                    )
                }
                ErrorKind::UnsupportedProperty(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(
                        f,
                        "UnsupportedProperty",
                        &__self_0,
                    )
                }
                ErrorKind::InvalidUTF8 => {
                    ::core::fmt::Formatter::write_str(f, "InvalidUTF8")
                }
                ErrorKind::InvalidPacketIdentifier => {
                    ::core::fmt::Formatter::write_str(f, "InvalidPacketIdentifier")
                }
            }
        }
    }
    #[automatically_derived]
    impl ::core::clone::Clone for ErrorKind {
        #[inline]
        fn clone(&self) -> ErrorKind {
            match self {
                ErrorKind::InsufficientData(__self_0, __self_1) => {
                    ErrorKind::InsufficientData(
                        ::core::clone::Clone::clone(__self_0),
                        ::core::clone::Clone::clone(__self_1),
                    )
                }
                ErrorKind::MalformedPacket => ErrorKind::MalformedPacket,
                ErrorKind::InvalidPacket => ErrorKind::InvalidPacket,
                ErrorKind::UnsupportedQosLevel => ErrorKind::UnsupportedQosLevel,
                ErrorKind::UnsupportedResponseType => ErrorKind::UnsupportedResponseType,
                ErrorKind::UnsupportedReason(__self_0) => {
                    ErrorKind::UnsupportedReason(::core::clone::Clone::clone(__self_0))
                }
                ErrorKind::UnsupportedProperty(__self_0) => {
                    ErrorKind::UnsupportedProperty(::core::clone::Clone::clone(__self_0))
                }
                ErrorKind::InvalidUTF8 => ErrorKind::InvalidUTF8,
                ErrorKind::InvalidPacketIdentifier => ErrorKind::InvalidPacketIdentifier,
            }
        }
    }
    #[automatically_derived]
    impl ::core::cmp::Eq for ErrorKind {
        #[inline]
        #[doc(hidden)]
        #[coverage(off)]
        fn assert_receiver_is_total_eq(&self) -> () {
            let _: ::core::cmp::AssertParamIsEq<usize>;
            let _: ::core::cmp::AssertParamIsEq<u8>;
        }
    }
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for ErrorKind {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for ErrorKind {
        #[inline]
        fn eq(&self, other: &ErrorKind) -> bool {
            let __self_discr = ::core::intrinsics::discriminant_value(self);
            let __arg1_discr = ::core::intrinsics::discriminant_value(other);
            __self_discr == __arg1_discr
                && match (self, other) {
                    (
                        ErrorKind::InsufficientData(__self_0, __self_1),
                        ErrorKind::InsufficientData(__arg1_0, __arg1_1),
                    ) => __self_0 == __arg1_0 && __self_1 == __arg1_1,
                    (
                        ErrorKind::UnsupportedReason(__self_0),
                        ErrorKind::UnsupportedReason(__arg1_0),
                    ) => __self_0 == __arg1_0,
                    (
                        ErrorKind::UnsupportedProperty(__self_0),
                        ErrorKind::UnsupportedProperty(__arg1_0),
                    ) => __self_0 == __arg1_0,
                    _ => true,
                }
        }
    }
    pub struct MqttCodecError {
        pub reason: String,
        pub kind: ErrorKind,
    }
    #[automatically_derived]
    impl ::core::default::Default for MqttCodecError {
        #[inline]
        fn default() -> MqttCodecError {
            MqttCodecError {
                reason: ::core::default::Default::default(),
                kind: ::core::default::Default::default(),
            }
        }
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for MqttCodecError {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::debug_struct_field2_finish(
                f,
                "MqttCodecError",
                "reason",
                &self.reason,
                "kind",
                &&self.kind,
            )
        }
    }
    impl Display for MqttCodecError {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("Mqtt codec error: {0}", self.reason))
        }
    }
    impl From<std::io::Error> for MqttCodecError {
        fn from(_err: std::io::Error) -> Self {
            MqttCodecError {
                reason: "IO error".to_string(),
                ..Default::default()
            }
        }
    }
    impl std::error::Error for MqttCodecError {}
    impl MqttCodecError {
        pub fn new(reason: &str) -> Self {
            MqttCodecError {
                reason: reason.to_string(),
                ..Default::default()
            }
        }
        pub fn new_with_kind(reason: &str, kind: ErrorKind) -> Self {
            MqttCodecError {
                reason: reason.to_string(),
                kind,
            }
        }
    }
    pub fn decode(src: &mut BytesMut) -> Result<Option<(Packet, u32)>, MqttCodecError> {
        let mut fixed_header = FixedHeader::default();
        let mut decode_len = fixed_header.decode(src)?;
        {
            ::std::io::_print(
                format_args!(
                    "Fixed header decoded: {0:?}, bytes read: {1}\n",
                    fixed_header,
                    decode_len,
                ),
            );
        };
        for idx in 1..=3 {
            if src[idx] & 0x80 != 0x00 {
                if src.remaining() < 1 {
                    return Err(
                        MqttCodecError::new_with_kind(
                            "Insufficient data",
                            ErrorKind::InsufficientData(1, src.remaining()),
                        ),
                    );
                }
            } else {
                break;
            }
        }
        let (packet_remaining, bytes_read) = decode_variable_byte_int(src)?;
        if src.remaining() < bytes_read as usize {
            return Err(
                MqttCodecError::new_with_kind(
                    "Insufficient data",
                    ErrorKind::InsufficientData(bytes_read as usize, src.remaining()),
                ),
            );
        }
        {
            ::std::io::_print(
                format_args!(
                    "Remaining length decoded: {0}, src remaining: {1}\n",
                    packet_remaining,
                    src.remaining(),
                ),
            );
        };
        decode_len += bytes_read;
        match src.remaining() {
            val if val < packet_remaining as usize => {
                return Err(MqttCodecError {
                    reason: ::alloc::__export::must_use({
                        ::alloc::fmt::format(
                            format_args!(
                                "malformed packet: remaining length actual: {0} expected: {1}",
                                val,
                                packet_remaining,
                            ),
                        )
                    }),
                    kind: ErrorKind::InsufficientData(packet_remaining as usize, val),
                });
            }
            val if val > packet_remaining as usize => {
                let total = src.remaining();
                let index = total - (total - packet_remaining as usize);
                _ = src.split_off(index);
            }
            _ => {}
        }
        match fixed_header.packet_type {
            PacketType::PingReq => {
                Ok(Some((Packet::PingRequest(crate::PingReq::default()), decode_len)))
            }
            PacketType::PingResp => {
                Ok(Some((Packet::PingResponse(crate::PingResp::default()), decode_len)))
            }
            PacketType::Connect => {
                let mut connect = Connect::new();
                decode_len += connect.decode(src)?;
                Ok(Some((Packet::Connect(Box::new(connect)), decode_len)))
            }
            PacketType::Publish => {
                let mut publish = Publish::new_from_header(fixed_header)?;
                decode_len += publish.decode(src)?;
                Ok(Some((Packet::Publish(publish), decode_len)))
            }
            PacketType::PubAck => {
                let mut puback = PubAck::default();
                decode_len += puback.decode(src)?;
                Ok(Some((Packet::PubAck(puback), decode_len)))
            }
            PacketType::PubComp => {
                let mut pubcomp = PubComp::default();
                decode_len += pubcomp.decode(src)?;
                Ok(Some((Packet::PubComp(pubcomp), decode_len)))
            }
            PacketType::PubRec => {
                let mut pubrec = PubRec::default();
                decode_len += pubrec.decode(src)?;
                Ok(Some((Packet::PubRec(pubrec), decode_len)))
            }
            PacketType::PubRel => {
                let mut pubrel = PubRel::default();
                pubrel.decode(src)?;
                Ok(Some((Packet::PubRel(pubrel), decode_len)))
            }
            PacketType::Disconnect => {
                let mut disconnect = Disconnect::default();
                decode_len += disconnect.decode(src)?;
                Ok(Some((Packet::Disconnect(disconnect), decode_len)))
            }
            PacketType::ConnAck => {
                let mut connack = ConnAck::default();
                decode_len += connack.decode(src)?;
                Ok(Some((Packet::ConnAck(connack), decode_len)))
            }
            PacketType::Subscribe => {
                let mut subscribe = Subscribe::default();
                decode_len += subscribe.decode(src)?;
                Ok(Some((Packet::Subscribe(subscribe), decode_len)))
            }
            PacketType::SubAck => {
                let mut suback = SubAck::default();
                decode_len += suback.decode(src)?;
                Ok(Some((Packet::SubAck(suback), decode_len)))
            }
            _ => Err(MqttCodecError::new("unsupported packet type")),
        }
    }
    impl Decode for String {
        fn decode(&mut self, src: &mut BytesMut) -> Result<u32, MqttCodecError> {
            if src.len() < 2 {
                return Err(MqttCodecError::new("malformed Mqtt packet: string length"));
            }
            let len = src.get_u16();
            let mut dest_vec = Vec::with_capacity(len as usize);
            let dest: &mut [u8] = &mut dest_vec[0..len as usize];
            src.try_copy_to_slice(dest)
                .map_err(|e| {
                    MqttCodecError::new_with_kind(
                        ::alloc::__export::must_use({
                                ::alloc::fmt::format(format_args!("{0:?}", e))
                            })
                            .as_str(),
                        ErrorKind::InsufficientData(len as usize, src.remaining()),
                    )
                })?;
            *self = String::from_utf8(dest_vec)
                .map_err(|e| MqttCodecError::new(
                    &::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!("{0:?}", e))
                    }),
                ))?;
            Ok(len as u32 + 2)
        }
    }
    impl Encode for Vec<u8> {
        fn encode(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
            encode_array_field(self, dest)
        }
    }
    impl Decode for Vec<u8> {
        fn decode(&mut self, src: &mut BytesMut) -> Result<u32, MqttCodecError> {
            if src.remaining() < 2 {
                return Err(MqttCodecError::new("Insufficient data for binary length"));
            }
            let len = src.get_u16() as usize;
            self.resize(len, 0);
            let dest_buf: &mut [u8] = &mut self[0..len];
            src.try_copy_to_slice(dest_buf)
                .map_err(|e| {
                    MqttCodecError::new_with_kind(
                        ::alloc::__export::must_use({
                                ::alloc::fmt::format(format_args!("{0:?}", e))
                            })
                            .as_str(),
                        ErrorKind::InsufficientData(len, src.remaining()),
                    )
                })?;
            Ok(len as u32 + 2)
        }
    }
    impl Decode for bool {
        fn decode(&mut self, src: &mut BytesMut) -> Result<u32, MqttCodecError> {
            *self = match src.get_u8() {
                0 => Ok(false),
                1 => Ok(true),
                v => {
                    Err(
                        MqttCodecError::new(
                            &::alloc::__export::must_use({
                                ::alloc::fmt::format(
                                    format_args!("invalid value {0} for boolean property", v),
                                )
                            }),
                        ),
                    )
                }
            }?;
            Ok(1)
        }
    }
    impl Decode for u8 {
        fn decode(&mut self, src: &mut BytesMut) -> Result<u32, MqttCodecError> {
            *self = src.get_u8();
            Ok(1)
        }
    }
    impl Decode for u16 {
        fn decode(&mut self, src: &mut BytesMut) -> Result<u32, MqttCodecError> {
            *self = src.get_u16();
            Ok(2)
        }
    }
    impl Decode for u32 {
        fn decode(&mut self, src: &mut BytesMut) -> Result<u32, MqttCodecError> {
            *self = src.get_u32();
            Ok(4)
        }
    }
    pub fn encode_string(src: &str, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
        let len = src.len();
        if len > u16::MAX as usize {
            return Err(MqttCodecError::new("string exceeds max length"));
        }
        dest.put_u16(len as u16);
        dest.put(src.as_bytes());
        Ok(())
    }
    pub fn decode_string(src: &mut BytesMut) -> Result<(String, u32), MqttCodecError> {
        let mut string = String::new();
        let bytes_read = string.decode(src)?;
        Ok((string, bytes_read))
    }
    pub fn encode_array_field(
        src: &[u8],
        dest: &mut BytesMut,
    ) -> Result<(), MqttCodecError> {
        let len = src.len();
        if len > u16::MAX as usize {
            return Err(MqttCodecError::new("binary data exceeds max length"));
        }
        dest.put_u16(len as u16);
        dest.put(src);
        Ok(())
    }
    pub fn decode_array_field(
        src: &mut BytesMut,
    ) -> Result<(Vec<u8>, u32), MqttCodecError> {
        let mut data = Vec::new();
        let bytes_read = data.decode(src)?;
        Ok((data, bytes_read))
    }
    /// Returns the length of an encoded MQTT variable length unsigned int
    pub fn variable_byte_int_size(value: u32) -> u32 {
        match value {
            0..=127 => 1,
            128..=16383 => 2,
            16384..=2097151 => 3,
            _ => 4,
        }
    }
    pub fn variable_byte_int_size_ref(value: &u32) -> u32 {
        variable_byte_int_size(*value)
    }
    pub fn codec_size_opt_variable_byte_int_ref(src: &Option<u32>) -> u32 {
        match src {
            Some(val) => variable_byte_int_size(*val),
            None => 0,
        }
    }
    pub fn decode_opt_variable_byte_int(
        src: &mut BytesMut,
    ) -> Result<(Option<u32>, u32), MqttCodecError> {
        let (val, bytes_read) = decode_variable_byte_int(src)?;
        Ok((Some(val), bytes_read))
    }
    pub fn decode_variable_byte_int(
        src: &mut BytesMut,
    ) -> Result<(u32, u32), MqttCodecError> {
        let mut result = 0_u32;
        let mut shift = 0;
        let mut next_byte = src.get_u8();
        let mut decode = true;
        while decode {
            result += ((next_byte & 0x7f) as u32) << shift;
            shift += 7;
            if next_byte & 0x80 == 0 {
                decode = false;
            } else {
                next_byte = src.get_u8();
            }
            if shift > 21 {
                return Err(
                    MqttCodecError::new("malformed packet: variable byte integer"),
                );
            }
        }
        Ok((result, shift / 7))
    }
    pub fn encode_variable_byte_int(
        val: u32,
        dest: &mut BytesMut,
    ) -> Result<(), MqttCodecError> {
        let mut encode = true;
        let mut input_val = val;
        while encode {
            let mut next_byte = (input_val % 0x80) as u8;
            input_val >>= 7;
            if input_val > 0 {
                next_byte |= 0x80;
            } else {
                encode = false;
            }
            dest.put_u8(next_byte);
        }
        Ok(())
    }
    pub fn encode_opt_variable_byte_int(
        val: Option<u32>,
        dest: &mut BytesMut,
    ) -> Result<(), MqttCodecError> {
        if let Some(v) = val { encode_variable_byte_int(v, dest) } else { Ok(()) }
    }
    pub fn encode_opt_variable_byte_int_ref(
        val: &Option<u32>,
        dest: &mut BytesMut,
    ) -> Result<(), MqttCodecError> {
        if let Some(v) = val { encode_variable_byte_int(*v, dest) } else { Ok(()) }
    }
    pub fn encode_variable_byte_int_ref(
        val: &u32,
        dest: &mut BytesMut,
    ) -> Result<(), MqttCodecError> {
        encode_variable_byte_int(*val, dest)
    }
    pub fn codec_size_vec_u8_raw(src: &Vec<u8>) -> u32 {
        src.len() as u32
    }
    pub fn encode_vec_u8_raw(
        src: &Vec<u8>,
        dest: &mut BytesMut,
    ) -> Result<(), MqttCodecError> {
        dest.put_slice(src);
        Ok(())
    }
    pub fn decode_vec_u8_raw(
        src: &mut BytesMut,
    ) -> Result<(Option<Vec<u8>>, u32), MqttCodecError> {
        let len = src.remaining();
        let mut dest = Vec::with_capacity(len);
        dest.resize(len, 0);
        let dest_buf: &mut [u8] = &mut dest[0..len];
        src.try_copy_to_slice(dest_buf)
            .map_err(|e| {
                MqttCodecError::new_with_kind(
                    ::alloc::__export::must_use({
                            ::alloc::fmt::format(format_args!("{0:?}", e))
                        })
                        .as_str(),
                    ErrorKind::InsufficientData(len, src.remaining()),
                )
            })?;
        Ok((Some(dest), len as u32))
    }
    pub fn codec_size_opt_vec_u8_raw(src: &Option<Vec<u8>>) -> u32 {
        match src {
            Some(vec) => vec.len() as u32,
            None => 0,
        }
    }
    pub fn encode_opt_vec_u8_raw(
        src: &Option<Vec<u8>>,
        dest: &mut BytesMut,
    ) -> Result<(), MqttCodecError> {
        if let Some(vec) = src {
            dest.put_slice(vec);
        }
        Ok(())
    }
    pub fn decode_opt_vec_u8_raw(
        src: &mut BytesMut,
        len: usize,
    ) -> Result<(Option<Vec<u8>>, u32), MqttCodecError> {
        let mut vec = Vec::new();
        vec.resize(len, 0);
        let dest_buf: &mut [u8] = &mut vec[0..len];
        src.try_copy_to_slice(dest_buf)
            .map_err(|e| {
                MqttCodecError::new_with_kind(
                    ::alloc::__export::must_use({
                            ::alloc::fmt::format(format_args!("{0:?}", e))
                        })
                        .as_str(),
                    ErrorKind::InsufficientData(len, src.remaining()),
                )
            })?;
        Ok((Some(vec), len as u32))
    }
}
pub mod connack {
    use crate::codec::{self, Reason};
    use crate::property::{PropertyType, UserProperty};
    use crate::{MqttCodecError, QoSLevel};
    use vaux_macro::packet;
    const CONNACK_SESSION_PRESENT_MASK: u8 = 0x01;
    pub struct ConnAck {
        pub fixed_header: codec::FixedHeader,
        ack_flags: u8,
        pub reason: Reason,
        pub session_expiry_interval: Option<u32>,
        pub receive_maximum: Option<u16>,
        pub maximum_qos: Option<QoSLevel>,
        pub retain_available: Option<bool>,
        pub maximum_packet_size: Option<u32>,
        pub assigned_client_id: Option<String>,
        pub topic_alias_maximum: Option<u16>,
        pub reason_string: Option<String>,
        pub subscription_identifier_available: Option<bool>,
        pub user_properties: UserProperty,
        pub wildcard_subscription_available: Option<bool>,
        pub shared_subscription_available: Option<bool>,
        pub server_keep_alive: Option<u16>,
        pub response_information: Option<String>,
        pub server_reference: Option<String>,
        pub auth_method: Option<String>,
        pub auth_data: Vec<u8>,
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for ConnAck {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            let names: &'static _ = &[
                "fixed_header",
                "ack_flags",
                "reason",
                "session_expiry_interval",
                "receive_maximum",
                "maximum_qos",
                "retain_available",
                "maximum_packet_size",
                "assigned_client_id",
                "topic_alias_maximum",
                "reason_string",
                "subscription_identifier_available",
                "user_properties",
                "wildcard_subscription_available",
                "shared_subscription_available",
                "server_keep_alive",
                "response_information",
                "server_reference",
                "auth_method",
                "auth_data",
            ];
            let values: &[&dyn ::core::fmt::Debug] = &[
                &self.fixed_header,
                &self.ack_flags,
                &self.reason,
                &self.session_expiry_interval,
                &self.receive_maximum,
                &self.maximum_qos,
                &self.retain_available,
                &self.maximum_packet_size,
                &self.assigned_client_id,
                &self.topic_alias_maximum,
                &self.reason_string,
                &self.subscription_identifier_available,
                &self.user_properties,
                &self.wildcard_subscription_available,
                &self.shared_subscription_available,
                &self.server_keep_alive,
                &self.response_information,
                &self.server_reference,
                &self.auth_method,
                &&self.auth_data,
            ];
            ::core::fmt::Formatter::debug_struct_fields_finish(
                f,
                "ConnAck",
                names,
                values,
            )
        }
    }
    #[automatically_derived]
    impl ::core::default::Default for ConnAck {
        #[inline]
        fn default() -> ConnAck {
            ConnAck {
                fixed_header: ::core::default::Default::default(),
                ack_flags: ::core::default::Default::default(),
                reason: ::core::default::Default::default(),
                session_expiry_interval: ::core::default::Default::default(),
                receive_maximum: ::core::default::Default::default(),
                maximum_qos: ::core::default::Default::default(),
                retain_available: ::core::default::Default::default(),
                maximum_packet_size: ::core::default::Default::default(),
                assigned_client_id: ::core::default::Default::default(),
                topic_alias_maximum: ::core::default::Default::default(),
                reason_string: ::core::default::Default::default(),
                subscription_identifier_available: ::core::default::Default::default(),
                user_properties: ::core::default::Default::default(),
                wildcard_subscription_available: ::core::default::Default::default(),
                shared_subscription_available: ::core::default::Default::default(),
                server_keep_alive: ::core::default::Default::default(),
                response_information: ::core::default::Default::default(),
                server_reference: ::core::default::Default::default(),
                auth_method: ::core::default::Default::default(),
                auth_data: ::core::default::Default::default(),
            }
        }
    }
    #[automatically_derived]
    impl ::core::clone::Clone for ConnAck {
        #[inline]
        fn clone(&self) -> ConnAck {
            ConnAck {
                fixed_header: ::core::clone::Clone::clone(&self.fixed_header),
                ack_flags: ::core::clone::Clone::clone(&self.ack_flags),
                reason: ::core::clone::Clone::clone(&self.reason),
                session_expiry_interval: ::core::clone::Clone::clone(
                    &self.session_expiry_interval,
                ),
                receive_maximum: ::core::clone::Clone::clone(&self.receive_maximum),
                maximum_qos: ::core::clone::Clone::clone(&self.maximum_qos),
                retain_available: ::core::clone::Clone::clone(&self.retain_available),
                maximum_packet_size: ::core::clone::Clone::clone(
                    &self.maximum_packet_size,
                ),
                assigned_client_id: ::core::clone::Clone::clone(
                    &self.assigned_client_id,
                ),
                topic_alias_maximum: ::core::clone::Clone::clone(
                    &self.topic_alias_maximum,
                ),
                reason_string: ::core::clone::Clone::clone(&self.reason_string),
                subscription_identifier_available: ::core::clone::Clone::clone(
                    &self.subscription_identifier_available,
                ),
                user_properties: ::core::clone::Clone::clone(&self.user_properties),
                wildcard_subscription_available: ::core::clone::Clone::clone(
                    &self.wildcard_subscription_available,
                ),
                shared_subscription_available: ::core::clone::Clone::clone(
                    &self.shared_subscription_available,
                ),
                server_keep_alive: ::core::clone::Clone::clone(&self.server_keep_alive),
                response_information: ::core::clone::Clone::clone(
                    &self.response_information,
                ),
                server_reference: ::core::clone::Clone::clone(&self.server_reference),
                auth_method: ::core::clone::Clone::clone(&self.auth_method),
                auth_data: ::core::clone::Clone::clone(&self.auth_data),
            }
        }
    }
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for ConnAck {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for ConnAck {
        #[inline]
        fn eq(&self, other: &ConnAck) -> bool {
            self.ack_flags == other.ack_flags && self.fixed_header == other.fixed_header
                && self.reason == other.reason
                && self.session_expiry_interval == other.session_expiry_interval
                && self.receive_maximum == other.receive_maximum
                && self.maximum_qos == other.maximum_qos
                && self.retain_available == other.retain_available
                && self.maximum_packet_size == other.maximum_packet_size
                && self.assigned_client_id == other.assigned_client_id
                && self.topic_alias_maximum == other.topic_alias_maximum
                && self.reason_string == other.reason_string
                && self.subscription_identifier_available
                    == other.subscription_identifier_available
                && self.user_properties == other.user_properties
                && self.wildcard_subscription_available
                    == other.wildcard_subscription_available
                && self.shared_subscription_available
                    == other.shared_subscription_available
                && self.server_keep_alive == other.server_keep_alive
                && self.response_information == other.response_information
                && self.server_reference == other.server_reference
                && self.auth_method == other.auth_method
                && self.auth_data == other.auth_data
        }
    }
    #[automatically_derived]
    impl ::core::cmp::Eq for ConnAck {
        #[inline]
        #[doc(hidden)]
        #[coverage(off)]
        fn assert_receiver_is_total_eq(&self) -> () {
            let _: ::core::cmp::AssertParamIsEq<codec::FixedHeader>;
            let _: ::core::cmp::AssertParamIsEq<u8>;
            let _: ::core::cmp::AssertParamIsEq<Reason>;
            let _: ::core::cmp::AssertParamIsEq<Option<u32>>;
            let _: ::core::cmp::AssertParamIsEq<Option<u16>>;
            let _: ::core::cmp::AssertParamIsEq<Option<QoSLevel>>;
            let _: ::core::cmp::AssertParamIsEq<Option<bool>>;
            let _: ::core::cmp::AssertParamIsEq<Option<u32>>;
            let _: ::core::cmp::AssertParamIsEq<Option<String>>;
            let _: ::core::cmp::AssertParamIsEq<Option<u16>>;
            let _: ::core::cmp::AssertParamIsEq<Option<String>>;
            let _: ::core::cmp::AssertParamIsEq<Option<bool>>;
            let _: ::core::cmp::AssertParamIsEq<UserProperty>;
            let _: ::core::cmp::AssertParamIsEq<Option<bool>>;
            let _: ::core::cmp::AssertParamIsEq<Option<bool>>;
            let _: ::core::cmp::AssertParamIsEq<Option<u16>>;
            let _: ::core::cmp::AssertParamIsEq<Option<String>>;
            let _: ::core::cmp::AssertParamIsEq<Option<String>>;
            let _: ::core::cmp::AssertParamIsEq<Option<String>>;
            let _: ::core::cmp::AssertParamIsEq<Vec<u8>>;
        }
    }
    impl ConnAck {
        pub fn new_with_fixed_header(
            fixed_header: codec::FixedHeader,
        ) -> Result<Self, codec::MqttCodecError> {
            if fixed_header.packet_type != codec::PacketType::ConnAck {
                return Err(
                    MqttCodecError::new(
                        ::alloc::__export::must_use({
                                ::alloc::fmt::format(
                                    format_args!(
                                        "Unsuppprted PacketType for {0}",
                                        "#struct_name",
                                    ),
                                )
                            })
                            .as_str(),
                    ),
                );
            }
            Ok(Self {
                fixed_header,
                ..Default::default()
            })
        }
    }
    impl codec::CodecSize for ConnAck {
        fn codec_size(&self) -> u32 {
            use codec::PropertyCodecSize;
            let mut total_size = 0;
            total_size += 1;
            total_size += self.reason.codec_size();
            let property_size = self.property_size();
            total_size + property_size + codec::variable_byte_int_size(property_size)
        }
    }
    impl codec::PropertyCodecSize for ConnAck {
        fn property_size(&self) -> u32 {
            use codec::CodecSize;
            let mut property_size = 0;
            if let Some(_) = &self.session_expiry_interval {
                property_size += 1 + 4;
            }
            if let Some(_) = &self.receive_maximum {
                property_size += 1 + 2;
            }
            if let Some(field) = &self.maximum_qos {
                property_size += field.codec_size();
            }
            if let Some(_) = &self.retain_available {
                property_size += 1 + 1;
            }
            if let Some(_) = &self.maximum_packet_size {
                property_size += 1 + 4;
            }
            if let Some(field_name) = &self.assigned_client_id {
                let value_size = field_name.len() as u32 + 2;
                property_size += 1 + value_size;
            }
            if let Some(_) = &self.topic_alias_maximum {
                property_size += 1 + 2;
            }
            if let Some(field_name) = &self.reason_string {
                let value_size = field_name.len() as u32 + 2;
                property_size += 1 + value_size;
            }
            if let Some(_) = &self.subscription_identifier_available {
                property_size += 1 + 1;
            }
            property_size += self.user_properties.codec_size();
            if let Some(_) = &self.wildcard_subscription_available {
                property_size += 1 + 1;
            }
            if let Some(_) = &self.shared_subscription_available {
                property_size += 1 + 1;
            }
            if let Some(_) = &self.server_keep_alive {
                property_size += 1 + 2;
            }
            if let Some(field_name) = &self.response_information {
                let value_size = field_name.len() as u32 + 2;
                property_size += 1 + value_size;
            }
            if let Some(field_name) = &self.server_reference {
                let value_size = field_name.len() as u32 + 2;
                property_size += 1 + value_size;
            }
            if let Some(field_name) = &self.auth_method {
                let value_size = field_name.len() as u32 + 2;
                property_size += 1 + value_size;
            }
            if !Vec::is_empty(&self.auth_data) {
                if !self.auth_data.is_empty() {
                    property_size += 1 + 2 + self.auth_data.len() as u32;
                }
            }
            property_size
        }
    }
    impl codec::Encode for ConnAck {
        fn encode(&self, dest: &mut bytes::BytesMut) -> Result<(), MqttCodecError> {
            use bytes::{BufMut, BytesMut};
            use codec::{CodecSize, PropertyCodecSize};
            self.fixed_header.encode(dest)?;
            codec::encode_variable_byte_int(self.codec_size(), dest)?;
            dest.put_u8(self.ack_flags);
            self.reason.encode(dest)?;
            codec::encode_variable_byte_int(self.property_size(), dest)?;
            if let Some(v) = self.session_expiry_interval {
                dest.put_u8(PropertyType::SessionExpiryInterval as u8);
                dest.put_u32(v);
            }
            if let Some(v) = self.receive_maximum {
                dest.put_u8(PropertyType::RecvMax as u8);
                dest.put_u16(v);
            }
            if let Some(maximum_qos) = self.maximum_qos.as_ref() {
                dest.put_u8(PropertyType::MaxQoS as u8);
                maximum_qos.encode(dest)?;
            }
            if let Some(v) = self.retain_available {
                dest.put_u8(PropertyType::RetainAvail as u8);
                dest.put_u8(if v { 1 } else { 0 });
            }
            if let Some(v) = self.maximum_packet_size {
                dest.put_u8(PropertyType::MaxPacketSize as u8);
                dest.put_u32(v);
            }
            if let Some(v) = self.assigned_client_id.as_ref() {
                dest.put_u8(PropertyType::AssignedClientId as u8);
                codec::encode_string(v, dest)?;
            }
            if let Some(v) = self.topic_alias_maximum {
                dest.put_u8(PropertyType::TopicAliasMax as u8);
                dest.put_u16(v);
            }
            if let Some(v) = self.reason_string.as_ref() {
                dest.put_u8(PropertyType::ReasonString as u8);
                codec::encode_string(v, dest)?;
            }
            if let Some(v) = self.subscription_identifier_available {
                dest.put_u8(PropertyType::SubIdAvail as u8);
                dest.put_u8(if v { 1 } else { 0 });
            }
            self.user_properties.encode(dest)?;
            if let Some(v) = self.wildcard_subscription_available {
                dest.put_u8(PropertyType::WildcardSubAvail as u8);
                dest.put_u8(if v { 1 } else { 0 });
            }
            if let Some(v) = self.shared_subscription_available {
                dest.put_u8(PropertyType::ShardSubAvail as u8);
                dest.put_u8(if v { 1 } else { 0 });
            }
            if let Some(v) = self.server_keep_alive {
                dest.put_u8(PropertyType::KeepAlive as u8);
                dest.put_u16(v);
            }
            if let Some(v) = self.response_information.as_ref() {
                dest.put_u8(PropertyType::RespInfo as u8);
                codec::encode_string(v, dest)?;
            }
            if let Some(v) = self.server_reference.as_ref() {
                dest.put_u8(PropertyType::ServerReference as u8);
                codec::encode_string(v, dest)?;
            }
            if let Some(v) = self.auth_method.as_ref() {
                dest.put_u8(PropertyType::AuthMethod as u8);
                codec::encode_string(v, dest)?;
            }
            if !Vec::is_empty(&self.auth_data) {
                dest.put_u8(PropertyType::AuthData as u8);
                codec::encode_array_field(&self.auth_data, dest)?;
            }
            Ok(())
        }
    }
    impl codec::Decode for ConnAck {
        fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<u32, MqttCodecError> {
            use bytes::{BufMut, Buf, BytesMut};
            let mut bytes_read = 0;
            self.ack_flags = src.get_u8();
            bytes_read += 1;
            bytes_read += self.reason.decode(src)?;
            let (property_length, var_bytes_read) = codec::decode_variable_byte_int(
                src,
            )?;
            bytes_read += var_bytes_read;
            let mut property_bytes_read = 0;
            while property_bytes_read < property_length {
                let property_type = src.get_u8().try_into()?;
                property_bytes_read += 1;
                match property_type {
                    PropertyType::SessionExpiryInterval => {
                        let mut value = u32::default();
                        property_bytes_read += value.decode(src)?;
                        self.session_expiry_interval = Some(value);
                    }
                    PropertyType::RecvMax => {
                        let mut value = u16::default();
                        property_bytes_read += value.decode(src)?;
                        self.receive_maximum = Some(value);
                    }
                    PropertyType::MaxQoS => {
                        let mut value = QoSLevel::default();
                        property_bytes_read += value.decode(src)?;
                        self.maximum_qos = Some(value);
                    }
                    PropertyType::RetainAvail => {
                        let mut value = bool::default();
                        property_bytes_read += value.decode(src)?;
                        self.retain_available = Some(value);
                    }
                    PropertyType::MaxPacketSize => {
                        let mut value = u32::default();
                        property_bytes_read += value.decode(src)?;
                        self.maximum_packet_size = Some(value);
                    }
                    PropertyType::AssignedClientId => {
                        let mut value = String::default();
                        property_bytes_read += value.decode(src)?;
                        self.assigned_client_id = Some(value);
                    }
                    PropertyType::TopicAliasMax => {
                        let mut value = u16::default();
                        property_bytes_read += value.decode(src)?;
                        self.topic_alias_maximum = Some(value);
                    }
                    PropertyType::ReasonString => {
                        let mut value = String::default();
                        property_bytes_read += value.decode(src)?;
                        self.reason_string = Some(value);
                    }
                    PropertyType::SubIdAvail => {
                        let mut value = bool::default();
                        property_bytes_read += value.decode(src)?;
                        self.subscription_identifier_available = Some(value);
                    }
                    PropertyType::UserProperty => {
                        property_bytes_read += self.user_properties.decode(src)?;
                    }
                    PropertyType::WildcardSubAvail => {
                        let mut value = bool::default();
                        property_bytes_read += value.decode(src)?;
                        self.wildcard_subscription_available = Some(value);
                    }
                    PropertyType::ShardSubAvail => {
                        let mut value = bool::default();
                        property_bytes_read += value.decode(src)?;
                        self.shared_subscription_available = Some(value);
                    }
                    PropertyType::KeepAlive => {
                        let mut value = u16::default();
                        property_bytes_read += value.decode(src)?;
                        self.server_keep_alive = Some(value);
                    }
                    PropertyType::RespInfo => {
                        let mut value = String::default();
                        property_bytes_read += value.decode(src)?;
                        self.response_information = Some(value);
                    }
                    PropertyType::ServerReference => {
                        let mut value = String::default();
                        property_bytes_read += value.decode(src)?;
                        self.server_reference = Some(value);
                    }
                    PropertyType::AuthMethod => {
                        let mut value = String::default();
                        property_bytes_read += value.decode(src)?;
                        self.auth_method = Some(value);
                    }
                    PropertyType::AuthData => {
                        let (value, var_bytes_read) = codec::decode_array_field(src)?;
                        bytes_read += var_bytes_read;
                        self.auth_data = value;
                    }
                    _ => {
                        return Err(
                            codec::MqttCodecError::new_with_kind(
                                ::alloc::__export::must_use({
                                        ::alloc::fmt::format(
                                            format_args!(
                                                "MQTT v5 property type {0:?} is not supported",
                                                property_type,
                                            ),
                                        )
                                    })
                                    .as_str(),
                                codec::ErrorKind::UnsupportedProperty(property_type as u8),
                            ),
                        );
                    }
                }
            }
            bytes_read += property_bytes_read;
            Ok(bytes_read)
        }
    }
    impl ConnAck {
        /// Returns true if the session is present flag is set in the CONNACK packet.
        ///
        pub fn session_present(&self) -> bool {
            (self.ack_flags & CONNACK_SESSION_PRESENT_MASK) != 0
        }
        /// Sets the session present flag in the CONNACK packet.
        pub fn set_session_present(&mut self, present: bool) {
            if present {
                self.ack_flags |= CONNACK_SESSION_PRESENT_MASK;
            } else {
                self.ack_flags &= !CONNACK_SESSION_PRESENT_MASK;
            }
        }
        pub fn set_session_expiry(&mut self, interval: u32) {
            if interval == 0 {
                self.session_expiry_interval = None;
                return;
            }
            self.session_expiry_interval = Some(interval);
        }
        pub fn set_server_keep_alive(&mut self, keep_alive: u16) {
            if keep_alive == 0 {
                self.server_keep_alive = None;
                return;
            }
            self.server_keep_alive = Some(keep_alive);
        }
        /// Gets the maximum number of QoS 1 and QoS 2 messages the session is willing to process.
        /// This is set as the CONNACK
        /// [Receive Maximum](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901049)
        /// property. The property is optional.
        pub fn receive_max(&self) -> Option<u16> {
            self.receive_maximum
        }
        /// Sets the maximum number of QoS 1 and QoS 2 messages the session is willing to process.
        /// This is set as the CONNACK
        /// [Receive Maximum](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901049)
        /// property.The property is optional and if set to 0, the property is cleared as it is a
        /// protocol error to have a value of 0.
        /// # Arguments
        /// max - The maximum number of QoS 1 and QoS 2 messages the session is willing to process.
        pub fn set_receive_max(&mut self, max: u16) {
            if max == 0 {
                self.receive_maximum = None;
                return;
            }
            self.receive_maximum = Some(max);
        }
    }
}
pub mod connect {
    use crate::{
        codec::self, property, will::WillHeader, MqttCodecError, PropertyType, QoSLevel,
        WillMessage,
    };
    use vaux_macro::packet;
    pub(crate) const CONNECT_FLAG_USERNAME: u8 = 0b_1000_0000;
    pub(crate) const CONNECT_FLAG_PASSWORD: u8 = 0b_0100_0000;
    pub(crate) const CONNECT_FLAG_WILL_RETAIN: u8 = 0b_0010_0000;
    pub(crate) const CONNECT_FLAG_WILL_QOS: u8 = 0b_0001_1000;
    pub(crate) const CONNECT_FLAG_WILL: u8 = 0b_0000_0100;
    pub(crate) const CONNECT_FLAG_CLEAN_START: u8 = 0b_0000_0010;
    const MQTT_PROTOCOL_NAME: &str = "MQTT";
    const MQTT_PROTOCOL_VERSION: u8 = 0x05;
    pub struct Connect {
        pub fixed_header: codec::FixedHeader,
        protocol_name: String,
        protocol_version: u8,
        connect_flags: u8,
        pub keep_alive: u16,
        pub session_expiry_interval: Option<u32>,
        pub receive_maximum: Option<u16>,
        pub max_packet_size: Option<u32>,
        pub topic_alias_maximum: Option<u16>,
        pub request_response_info: Option<bool>,
        pub request_problem_info: Option<bool>,
        pub auth_method: Option<String>,
        pub auth_data: Vec<u8>,
        pub user_properties: property::UserProperty,
        pub client_id: String,
        pub(crate) will_properties: Option<WillHeader>,
        pub(crate) will_payload: Vec<u8>,
        pub(crate) will_topic: Option<String>,
        username: String,
        password: Vec<u8>,
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for Connect {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            let names: &'static _ = &[
                "fixed_header",
                "protocol_name",
                "protocol_version",
                "connect_flags",
                "keep_alive",
                "session_expiry_interval",
                "receive_maximum",
                "max_packet_size",
                "topic_alias_maximum",
                "request_response_info",
                "request_problem_info",
                "auth_method",
                "auth_data",
                "user_properties",
                "client_id",
                "will_properties",
                "will_payload",
                "will_topic",
                "username",
                "password",
            ];
            let values: &[&dyn ::core::fmt::Debug] = &[
                &self.fixed_header,
                &self.protocol_name,
                &self.protocol_version,
                &self.connect_flags,
                &self.keep_alive,
                &self.session_expiry_interval,
                &self.receive_maximum,
                &self.max_packet_size,
                &self.topic_alias_maximum,
                &self.request_response_info,
                &self.request_problem_info,
                &self.auth_method,
                &self.auth_data,
                &self.user_properties,
                &self.client_id,
                &self.will_properties,
                &self.will_payload,
                &self.will_topic,
                &self.username,
                &&self.password,
            ];
            ::core::fmt::Formatter::debug_struct_fields_finish(
                f,
                "Connect",
                names,
                values,
            )
        }
    }
    #[automatically_derived]
    impl ::core::clone::Clone for Connect {
        #[inline]
        fn clone(&self) -> Connect {
            Connect {
                fixed_header: ::core::clone::Clone::clone(&self.fixed_header),
                protocol_name: ::core::clone::Clone::clone(&self.protocol_name),
                protocol_version: ::core::clone::Clone::clone(&self.protocol_version),
                connect_flags: ::core::clone::Clone::clone(&self.connect_flags),
                keep_alive: ::core::clone::Clone::clone(&self.keep_alive),
                session_expiry_interval: ::core::clone::Clone::clone(
                    &self.session_expiry_interval,
                ),
                receive_maximum: ::core::clone::Clone::clone(&self.receive_maximum),
                max_packet_size: ::core::clone::Clone::clone(&self.max_packet_size),
                topic_alias_maximum: ::core::clone::Clone::clone(
                    &self.topic_alias_maximum,
                ),
                request_response_info: ::core::clone::Clone::clone(
                    &self.request_response_info,
                ),
                request_problem_info: ::core::clone::Clone::clone(
                    &self.request_problem_info,
                ),
                auth_method: ::core::clone::Clone::clone(&self.auth_method),
                auth_data: ::core::clone::Clone::clone(&self.auth_data),
                user_properties: ::core::clone::Clone::clone(&self.user_properties),
                client_id: ::core::clone::Clone::clone(&self.client_id),
                will_properties: ::core::clone::Clone::clone(&self.will_properties),
                will_payload: ::core::clone::Clone::clone(&self.will_payload),
                will_topic: ::core::clone::Clone::clone(&self.will_topic),
                username: ::core::clone::Clone::clone(&self.username),
                password: ::core::clone::Clone::clone(&self.password),
            }
        }
    }
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for Connect {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for Connect {
        #[inline]
        fn eq(&self, other: &Connect) -> bool {
            self.protocol_version == other.protocol_version
                && self.connect_flags == other.connect_flags
                && self.keep_alive == other.keep_alive
                && self.fixed_header == other.fixed_header
                && self.protocol_name == other.protocol_name
                && self.session_expiry_interval == other.session_expiry_interval
                && self.receive_maximum == other.receive_maximum
                && self.max_packet_size == other.max_packet_size
                && self.topic_alias_maximum == other.topic_alias_maximum
                && self.request_response_info == other.request_response_info
                && self.request_problem_info == other.request_problem_info
                && self.auth_method == other.auth_method
                && self.auth_data == other.auth_data
                && self.user_properties == other.user_properties
                && self.client_id == other.client_id
                && self.will_properties == other.will_properties
                && self.will_payload == other.will_payload
                && self.will_topic == other.will_topic && self.username == other.username
                && self.password == other.password
        }
    }
    #[automatically_derived]
    impl ::core::cmp::Eq for Connect {
        #[inline]
        #[doc(hidden)]
        #[coverage(off)]
        fn assert_receiver_is_total_eq(&self) -> () {
            let _: ::core::cmp::AssertParamIsEq<codec::FixedHeader>;
            let _: ::core::cmp::AssertParamIsEq<String>;
            let _: ::core::cmp::AssertParamIsEq<u8>;
            let _: ::core::cmp::AssertParamIsEq<u16>;
            let _: ::core::cmp::AssertParamIsEq<Option<u32>>;
            let _: ::core::cmp::AssertParamIsEq<Option<u16>>;
            let _: ::core::cmp::AssertParamIsEq<Option<u32>>;
            let _: ::core::cmp::AssertParamIsEq<Option<u16>>;
            let _: ::core::cmp::AssertParamIsEq<Option<bool>>;
            let _: ::core::cmp::AssertParamIsEq<Option<bool>>;
            let _: ::core::cmp::AssertParamIsEq<Option<String>>;
            let _: ::core::cmp::AssertParamIsEq<Vec<u8>>;
            let _: ::core::cmp::AssertParamIsEq<property::UserProperty>;
            let _: ::core::cmp::AssertParamIsEq<Option<WillHeader>>;
            let _: ::core::cmp::AssertParamIsEq<Vec<u8>>;
            let _: ::core::cmp::AssertParamIsEq<Option<String>>;
            let _: ::core::cmp::AssertParamIsEq<Vec<u8>>;
        }
    }
    impl Connect {
        pub fn new_with_fixed_header(
            fixed_header: codec::FixedHeader,
        ) -> Result<Self, codec::MqttCodecError> {
            if fixed_header.packet_type != codec::PacketType::Connect {
                return Err(
                    MqttCodecError::new(
                        ::alloc::__export::must_use({
                                ::alloc::fmt::format(
                                    format_args!(
                                        "Unsuppprted PacketType for {0}",
                                        "#struct_name",
                                    ),
                                )
                            })
                            .as_str(),
                    ),
                );
            }
            Ok(Self {
                fixed_header,
                ..Default::default()
            })
        }
    }
    impl codec::CodecSize for Connect {
        fn codec_size(&self) -> u32 {
            use codec::PropertyCodecSize;
            let mut total_size = 0;
            total_size += 2;
            total_size += self.protocol_name.len() as u32;
            total_size += 1;
            total_size += 1;
            total_size += 2;
            total_size += 2;
            total_size += self.client_id.len() as u32;
            if let Some(field) = &self.will_properties {
                total_size += field.codec_size();
            }
            if !Vec::is_empty(&self.will_payload) {
                if !self.will_payload.is_empty() {
                    total_size += 2 + self.will_payload.len() as u32;
                }
            }
            if let Some(value) = &self.will_topic {
                total_size += 2;
                total_size += value.len() as u32;
            }
            if !String::is_empty(&self.username) {
                total_size += 2;
                total_size += self.username.len() as u32;
            }
            if !Vec::is_empty(&self.password) {
                if !self.password.is_empty() {
                    total_size += 2 + self.password.len() as u32;
                }
            }
            let property_size = self.property_size();
            total_size + property_size + codec::variable_byte_int_size(property_size)
        }
    }
    impl codec::PropertyCodecSize for Connect {
        fn property_size(&self) -> u32 {
            use codec::CodecSize;
            let mut property_size = 0;
            if let Some(_) = &self.session_expiry_interval {
                property_size += 1 + 4;
            }
            if let Some(_) = &self.receive_maximum {
                property_size += 1 + 2;
            }
            if let Some(_) = &self.max_packet_size {
                property_size += 1 + 4;
            }
            if let Some(_) = &self.topic_alias_maximum {
                property_size += 1 + 2;
            }
            if let Some(_) = &self.request_response_info {
                property_size += 1 + 1;
            }
            if let Some(_) = &self.request_problem_info {
                property_size += 1 + 1;
            }
            if let Some(field_name) = &self.auth_method {
                let value_size = field_name.len() as u32 + 2;
                property_size += 1 + value_size;
            }
            if !Vec::is_empty(&self.auth_data) {
                if !self.auth_data.is_empty() {
                    property_size += 1 + 2 + self.auth_data.len() as u32;
                }
            }
            property_size += self.user_properties.codec_size();
            property_size
        }
    }
    impl codec::Encode for Connect {
        fn encode(&self, dest: &mut bytes::BytesMut) -> Result<(), MqttCodecError> {
            use bytes::{BufMut, BytesMut};
            use codec::{CodecSize, PropertyCodecSize};
            self.fixed_header.encode(dest)?;
            codec::encode_variable_byte_int(self.codec_size(), dest)?;
            codec::encode_string(&self.protocol_name, dest)?;
            dest.put_u8(self.protocol_version);
            dest.put_u8(self.connect_flags);
            dest.put_u16(self.keep_alive);
            codec::encode_variable_byte_int(self.property_size(), dest)?;
            if let Some(v) = self.session_expiry_interval {
                dest.put_u8(PropertyType::SessionExpiryInterval as u8);
                dest.put_u32(v);
            }
            if let Some(v) = self.receive_maximum {
                dest.put_u8(PropertyType::RecvMax as u8);
                dest.put_u16(v);
            }
            if let Some(v) = self.max_packet_size {
                dest.put_u8(PropertyType::MaxPacketSize as u8);
                dest.put_u32(v);
            }
            if let Some(v) = self.topic_alias_maximum {
                dest.put_u8(PropertyType::TopicAliasMax as u8);
                dest.put_u16(v);
            }
            if let Some(v) = self.request_response_info {
                dest.put_u8(PropertyType::ReqRespInfo as u8);
                dest.put_u8(if v { 1 } else { 0 });
            }
            if let Some(v) = self.request_problem_info {
                dest.put_u8(PropertyType::ReqProblemInfo as u8);
                dest.put_u8(if v { 1 } else { 0 });
            }
            if let Some(v) = self.auth_method.as_ref() {
                dest.put_u8(PropertyType::AuthMethod as u8);
                codec::encode_string(v, dest)?;
            }
            if !Vec::is_empty(&self.auth_data) {
                dest.put_u8(PropertyType::AuthData as u8);
                codec::encode_array_field(&self.auth_data, dest)?;
            }
            self.user_properties.encode(dest)?;
            codec::encode_string(&self.client_id, dest)?;
            if let Some(will_properties) = self.will_properties.as_ref() {
                will_properties.encode(dest)?;
            }
            if !Vec::is_empty(&self.will_payload) {
                codec::encode_array_field(&self.will_payload, dest)?;
            }
            if let Some(v) = self.will_topic.as_ref() {
                codec::encode_string(v, dest)?;
            }
            if !String::is_empty(&self.username) {
                codec::encode_string(&self.username, dest)?;
            }
            if !Vec::is_empty(&self.password) {
                codec::encode_array_field(&self.password, dest)?;
            }
            Ok(())
        }
    }
    impl codec::Decode for Connect {
        fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<u32, MqttCodecError> {
            use bytes::{BufMut, Buf, BytesMut};
            let mut bytes_read = 0;
            bytes_read += self.protocol_name.decode(src)?;
            self.protocol_version = src.get_u8();
            bytes_read += 1;
            self.connect_flags = src.get_u8();
            bytes_read += 1;
            self.keep_alive = src.get_u16();
            bytes_read += 2;
            let (property_length, var_bytes_read) = codec::decode_variable_byte_int(
                src,
            )?;
            bytes_read += var_bytes_read;
            let mut property_bytes_read = 0;
            while property_bytes_read < property_length {
                let property_type = src.get_u8().try_into()?;
                property_bytes_read += 1;
                match property_type {
                    PropertyType::SessionExpiryInterval => {
                        let mut value = u32::default();
                        property_bytes_read += value.decode(src)?;
                        self.session_expiry_interval = Some(value);
                    }
                    PropertyType::RecvMax => {
                        let mut value = u16::default();
                        property_bytes_read += value.decode(src)?;
                        self.receive_maximum = Some(value);
                    }
                    PropertyType::MaxPacketSize => {
                        let mut value = u32::default();
                        property_bytes_read += value.decode(src)?;
                        self.max_packet_size = Some(value);
                    }
                    PropertyType::TopicAliasMax => {
                        let mut value = u16::default();
                        property_bytes_read += value.decode(src)?;
                        self.topic_alias_maximum = Some(value);
                    }
                    PropertyType::ReqRespInfo => {
                        let mut value = bool::default();
                        property_bytes_read += value.decode(src)?;
                        self.request_response_info = Some(value);
                    }
                    PropertyType::ReqProblemInfo => {
                        let mut value = bool::default();
                        property_bytes_read += value.decode(src)?;
                        self.request_problem_info = Some(value);
                    }
                    PropertyType::AuthMethod => {
                        let mut value = String::default();
                        property_bytes_read += value.decode(src)?;
                        self.auth_method = Some(value);
                    }
                    PropertyType::AuthData => {
                        let (value, var_bytes_read) = codec::decode_array_field(src)?;
                        bytes_read += var_bytes_read;
                        self.auth_data = value;
                    }
                    PropertyType::UserProperty => {
                        property_bytes_read += self.user_properties.decode(src)?;
                    }
                    _ => {
                        return Err(
                            codec::MqttCodecError::new_with_kind(
                                ::alloc::__export::must_use({
                                        ::alloc::fmt::format(
                                            format_args!(
                                                "MQTT v5 property type {0:?} is not supported",
                                                property_type,
                                            ),
                                        )
                                    })
                                    .as_str(),
                                codec::ErrorKind::UnsupportedProperty(property_type as u8),
                            ),
                        );
                    }
                }
            }
            bytes_read += property_bytes_read;
            Ok(bytes_read)
        }
    }
    impl Default for Connect {
        fn default() -> Self {
            Self::new()
        }
    }
    impl Connect {
        /// Creates a new ConnectHeader with default values. The protocol name is set to "MQTT"
        /// and the protocol version is set to 5. The protocol version is not configurable in
        /// this implementation.
        ///  Conntect Flags:
        /// |
        pub fn new() -> Self {
            Connect {
                fixed_header: codec::FixedHeader::new(codec::PacketType::Connect),
                protocol_name: MQTT_PROTOCOL_NAME.to_string(),
                protocol_version: MQTT_PROTOCOL_VERSION,
                connect_flags: 0,
                keep_alive: 60,
                session_expiry_interval: None,
                receive_maximum: None,
                max_packet_size: None,
                topic_alias_maximum: None,
                request_response_info: None,
                request_problem_info: None,
                auth_method: None,
                auth_data: Vec::new(),
                user_properties: property::UserProperty::new(),
                client_id: uuid::Uuid::new_v4().to_string(),
                will_properties: None,
                will_payload: Vec::new(),
                will_topic: None,
                username: String::new(),
                password: Vec::new(),
            }
        }
        pub fn clean_start(&self) -> bool {
            (self.connect_flags & CONNECT_FLAG_CLEAN_START) != 0
        }
        pub fn set_clean_start(&mut self, clean_start: bool) {
            self.connect_flags = (self.connect_flags & !CONNECT_FLAG_CLEAN_START)
                | (clean_start as u8) << 1;
        }
        pub fn will_retain(&self) -> bool {
            (self.connect_flags & CONNECT_FLAG_WILL_RETAIN) != 0
        }
        pub(crate) fn set_will_retain(&mut self, will_retain: bool) {
            self.connect_flags = (self.connect_flags & !CONNECT_FLAG_WILL_RETAIN)
                | (will_retain as u8) << 5;
        }
        pub fn will_qos(&self) -> Result<QoSLevel, MqttCodecError> {
            ((self.connect_flags & CONNECT_FLAG_WILL_QOS) >> 3).try_into()
        }
        pub(crate) fn set_will_qos(&mut self, qos: QoSLevel) {
            self.connect_flags = (self.connect_flags & !CONNECT_FLAG_WILL_QOS)
                | ((qos as u8) << 3);
        }
        pub fn will(&self) -> bool {
            (self.connect_flags & CONNECT_FLAG_WILL) != 0
        }
        pub(crate) fn set_will(&mut self, will: bool) {
            self.connect_flags = (self.connect_flags & !CONNECT_FLAG_WILL)
                | (will as u8) << 2;
        }
        pub fn username(&self) -> bool {
            (self.connect_flags & CONNECT_FLAG_USERNAME) != 0
        }
        pub fn set_username(&mut self, username: Option<String>) {
            if username.is_none() {
                self.connect_flags = self.connect_flags & !CONNECT_FLAG_USERNAME;
            } else {
                self.connect_flags = (self.connect_flags & !CONNECT_FLAG_USERNAME)
                    | (1 as u8) << 7;
            }
            self.username = username.unwrap_or_default();
        }
        pub fn password(&self) -> bool {
            (self.connect_flags & CONNECT_FLAG_PASSWORD) != 0
        }
        pub fn set_password(&mut self, password: Option<Vec<u8>>) {
            if password.is_none() {
                self.connect_flags = self.connect_flags & !CONNECT_FLAG_PASSWORD;
            } else {
                self.connect_flags = (self.connect_flags & !CONNECT_FLAG_PASSWORD)
                    | (1 as u8) << 6;
            }
            self.password = password.unwrap_or_default();
        }
        pub fn set_session_expiry_interval(&mut self, interval: u32) {
            if interval == 0 {
                self.session_expiry_interval = None;
            } else {
                self.session_expiry_interval = Some(interval);
            }
        }
        pub fn will_message(&self) -> Option<WillMessage> {
            if self.will() {
                Some(WillMessage {
                    topic: self.will_topic.clone().unwrap_or_default(),
                    payload: self.will_payload.clone(),
                    qos: self.will_qos().unwrap_or(QoSLevel::AtMostOnce),
                    retain: self.will_retain(),
                    header: self.will_properties.clone().unwrap_or_default(),
                })
            } else {
                None
            }
        }
        pub fn set_will_message(&mut self, will: WillMessage) {
            self.will_topic = Some(will.topic);
            self.will_payload = will.payload;
            self.will_properties = Some(will.header);
            self.set_will(true);
            self.set_will_qos(will.qos);
            self.set_will_retain(will.retain);
        }
    }
}
pub mod disconnect {
    use crate::{codec, property::UserProperty, MqttCodecError, PropertyType, Reason};
    use vaux_macro::packet;
    pub struct Disconnect {
        pub fixed_header: codec::FixedHeader,
        pub reason: Reason,
        pub session_expiry_interval: Option<u32>,
        pub reason_string: Option<String>,
        pub server_reference: Option<String>,
        pub user_properties: UserProperty,
    }
    #[automatically_derived]
    impl ::core::clone::Clone for Disconnect {
        #[inline]
        fn clone(&self) -> Disconnect {
            Disconnect {
                fixed_header: ::core::clone::Clone::clone(&self.fixed_header),
                reason: ::core::clone::Clone::clone(&self.reason),
                session_expiry_interval: ::core::clone::Clone::clone(
                    &self.session_expiry_interval,
                ),
                reason_string: ::core::clone::Clone::clone(&self.reason_string),
                server_reference: ::core::clone::Clone::clone(&self.server_reference),
                user_properties: ::core::clone::Clone::clone(&self.user_properties),
            }
        }
    }
    #[automatically_derived]
    impl ::core::default::Default for Disconnect {
        #[inline]
        fn default() -> Disconnect {
            Disconnect {
                fixed_header: ::core::default::Default::default(),
                reason: ::core::default::Default::default(),
                session_expiry_interval: ::core::default::Default::default(),
                reason_string: ::core::default::Default::default(),
                server_reference: ::core::default::Default::default(),
                user_properties: ::core::default::Default::default(),
            }
        }
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for Disconnect {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            let names: &'static _ = &[
                "fixed_header",
                "reason",
                "session_expiry_interval",
                "reason_string",
                "server_reference",
                "user_properties",
            ];
            let values: &[&dyn ::core::fmt::Debug] = &[
                &self.fixed_header,
                &self.reason,
                &self.session_expiry_interval,
                &self.reason_string,
                &self.server_reference,
                &&self.user_properties,
            ];
            ::core::fmt::Formatter::debug_struct_fields_finish(
                f,
                "Disconnect",
                names,
                values,
            )
        }
    }
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for Disconnect {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for Disconnect {
        #[inline]
        fn eq(&self, other: &Disconnect) -> bool {
            self.fixed_header == other.fixed_header && self.reason == other.reason
                && self.session_expiry_interval == other.session_expiry_interval
                && self.reason_string == other.reason_string
                && self.server_reference == other.server_reference
                && self.user_properties == other.user_properties
        }
    }
    #[automatically_derived]
    impl ::core::cmp::Eq for Disconnect {
        #[inline]
        #[doc(hidden)]
        #[coverage(off)]
        fn assert_receiver_is_total_eq(&self) -> () {
            let _: ::core::cmp::AssertParamIsEq<codec::FixedHeader>;
            let _: ::core::cmp::AssertParamIsEq<Reason>;
            let _: ::core::cmp::AssertParamIsEq<Option<u32>>;
            let _: ::core::cmp::AssertParamIsEq<Option<String>>;
            let _: ::core::cmp::AssertParamIsEq<Option<String>>;
            let _: ::core::cmp::AssertParamIsEq<UserProperty>;
        }
    }
    impl Disconnect {
        pub fn new_with_fixed_header(
            fixed_header: codec::FixedHeader,
        ) -> Result<Self, codec::MqttCodecError> {
            if fixed_header.packet_type != codec::PacketType::Disconnect {
                return Err(
                    MqttCodecError::new(
                        ::alloc::__export::must_use({
                                ::alloc::fmt::format(
                                    format_args!(
                                        "Unsuppprted PacketType for {0}",
                                        "#struct_name",
                                    ),
                                )
                            })
                            .as_str(),
                    ),
                );
            }
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
            property_size += self.user_properties.codec_size();
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
        fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<u32, MqttCodecError> {
            use bytes::{BufMut, Buf, BytesMut};
            let mut bytes_read = 0;
            bytes_read += self.reason.decode(src)?;
            let (property_length, var_bytes_read) = codec::decode_variable_byte_int(
                src,
            )?;
            bytes_read += var_bytes_read;
            let mut property_bytes_read = 0;
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
                                ::alloc::__export::must_use({
                                        ::alloc::fmt::format(
                                            format_args!(
                                                "MQTT v5 property type {0:?} is not supported",
                                                property_type,
                                            ),
                                        )
                                    })
                                    .as_str(),
                                codec::ErrorKind::UnsupportedProperty(property_type as u8),
                            ),
                        );
                    }
                }
            }
            bytes_read += property_bytes_read;
            Ok(bytes_read)
        }
    }
    impl Disconnect {
        pub fn new(reason: Reason) -> Self {
            Self {
                reason,
                ..Default::default()
            }
        }
    }
}
pub mod property {
    use crate::{
        codec::{self, encode_string},
        MqttCodecError,
    };
    use bytes::{BufMut, BytesMut};
    use std::{collections::HashMap, fmt::{Display, Formatter}};
    /// MQTT property type. For more information on the specific property types,
    /// please see the
    /// [MQTT Specification](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901027).
    /// All of the types below are MQTT protocol types.
    /// Identifier | Name | Type
    /// -----------+------+-----
    /// 0x01 | Payload Format Indicator | byte
    /// 0x02 | Message Expiry Interval | 4 byte Integer
    /// 0x03 | Content Type | UTF-8 string
    /// 0x08 | Response Topic | UTF-8 string
    /// 0x09 | Correlation Data | binary data
    /// 0x0b | Subscription Identifier | Variable Length Integer
    /// 0x11 | Session Expiry Interval | 4 byte Integer
    /// 0x12 | Assigned Client Identifier | UTF-8 string
    /// 0x13 | Server Keep Alive | 2 byte integer
    /// 0x15 | Authentication Method | UTF-8 string
    /// 0x16 | Authentication Data | binary data
    /// 0x17 | Request Problem Information | byte
    /// 0x18 | Will Delay Interval | 4 byte integer
    /// 0x19 | Request Response Information | byte
    /// 0x1a | Response Information | UTF-8 string
    /// 0x1c | Server Reference | UTF-8 string
    /// 0x1f | Reason String | UTF-8 string
    /// 0x21 | Receive Maximum | 2 byte integer
    /// 0x22 | Topic Alias Maximum | 2 byte integer
    /// 0x23 | Topic Alias | 2 byte integer
    /// 0x24 | Maximum QoS | byte
    /// 0x25 | Retain Available | byte
    /// 0x26 | User Property | UTF-8 string pair
    /// 0x27 | Maximum Packet Size | 4 byte integer
    /// 0x28 | Wildcard Subscription Available | byte
    /// 0x29 | Subscription Identifier Available | byte
    /// 0x2a | Shared Subscription Available | byte
    #[repr(u8)]
    pub enum PropertyType {
        PayloadFormat = 0x01,
        MessageExpiry = 0x02,
        ContentType = 0x03,
        ResponseTopic = 0x08,
        CorrelationData = 0x09,
        SubscriptionIdentifier = 0x0b,
        SessionExpiryInterval = 0x11,
        AssignedClientId = 0x12,
        KeepAlive = 0x13,
        AuthMethod = 0x15,
        AuthData = 0x16,
        ReqProblemInfo = 0x17,
        WillDelay = 0x18,
        ReqRespInfo = 0x19,
        RespInfo = 0x1a,
        ServerReference = 0x1c,
        ReasonString = 0x1f,
        RecvMax = 0x21,
        TopicAliasMax = 0x22,
        TopicAlias = 0x23,
        MaxQoS = 0x24,
        RetainAvail = 0x25,
        UserProperty = 0x26,
        MaxPacketSize = 0x27,
        WildcardSubAvail = 0x28,
        SubIdAvail = 0x29,
        ShardSubAvail = 0x2a,
    }
    #[automatically_derived]
    impl ::core::hash::Hash for PropertyType {
        #[inline]
        fn hash<__H: ::core::hash::Hasher>(&self, state: &mut __H) -> () {
            let __self_discr = ::core::intrinsics::discriminant_value(self);
            ::core::hash::Hash::hash(&__self_discr, state)
        }
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for PropertyType {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::write_str(
                f,
                match self {
                    PropertyType::PayloadFormat => "PayloadFormat",
                    PropertyType::MessageExpiry => "MessageExpiry",
                    PropertyType::ContentType => "ContentType",
                    PropertyType::ResponseTopic => "ResponseTopic",
                    PropertyType::CorrelationData => "CorrelationData",
                    PropertyType::SubscriptionIdentifier => "SubscriptionIdentifier",
                    PropertyType::SessionExpiryInterval => "SessionExpiryInterval",
                    PropertyType::AssignedClientId => "AssignedClientId",
                    PropertyType::KeepAlive => "KeepAlive",
                    PropertyType::AuthMethod => "AuthMethod",
                    PropertyType::AuthData => "AuthData",
                    PropertyType::ReqProblemInfo => "ReqProblemInfo",
                    PropertyType::WillDelay => "WillDelay",
                    PropertyType::ReqRespInfo => "ReqRespInfo",
                    PropertyType::RespInfo => "RespInfo",
                    PropertyType::ServerReference => "ServerReference",
                    PropertyType::ReasonString => "ReasonString",
                    PropertyType::RecvMax => "RecvMax",
                    PropertyType::TopicAliasMax => "TopicAliasMax",
                    PropertyType::TopicAlias => "TopicAlias",
                    PropertyType::MaxQoS => "MaxQoS",
                    PropertyType::RetainAvail => "RetainAvail",
                    PropertyType::UserProperty => "UserProperty",
                    PropertyType::MaxPacketSize => "MaxPacketSize",
                    PropertyType::WildcardSubAvail => "WildcardSubAvail",
                    PropertyType::SubIdAvail => "SubIdAvail",
                    PropertyType::ShardSubAvail => "ShardSubAvail",
                },
            )
        }
    }
    #[automatically_derived]
    impl ::core::marker::Copy for PropertyType {}
    #[automatically_derived]
    impl ::core::clone::Clone for PropertyType {
        #[inline]
        fn clone(&self) -> PropertyType {
            *self
        }
    }
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for PropertyType {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for PropertyType {
        #[inline]
        fn eq(&self, other: &PropertyType) -> bool {
            let __self_discr = ::core::intrinsics::discriminant_value(self);
            let __arg1_discr = ::core::intrinsics::discriminant_value(other);
            __self_discr == __arg1_discr
        }
    }
    #[automatically_derived]
    impl ::core::cmp::Eq for PropertyType {
        #[inline]
        #[doc(hidden)]
        #[coverage(off)]
        fn assert_receiver_is_total_eq(&self) -> () {}
    }
    impl Display for PropertyType {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            match self {
                PropertyType::PayloadFormat => {
                    f.write_fmt(format_args!("\"Payload Format Indicator\""))
                }
                PropertyType::MessageExpiry => {
                    f.write_fmt(format_args!("\"Message Expiry Interval\""))
                }
                PropertyType::ContentType => {
                    f.write_fmt(format_args!("\"Content Type\""))
                }
                PropertyType::ResponseTopic => {
                    f.write_fmt(format_args!("\"Response Topic\""))
                }
                PropertyType::CorrelationData => {
                    f.write_fmt(format_args!("\"Correlation Data\""))
                }
                PropertyType::SubscriptionIdentifier => {
                    f.write_fmt(format_args!("\"Subscription Identifier\""))
                }
                PropertyType::SessionExpiryInterval => {
                    f.write_fmt(format_args!("\"Session Expiry Interval\""))
                }
                PropertyType::AssignedClientId => {
                    f.write_fmt(format_args!("\"Assigned Client Identifier\""))
                }
                PropertyType::KeepAlive => {
                    f.write_fmt(format_args!("\"Server Keep Alive\""))
                }
                PropertyType::AuthMethod => {
                    f.write_fmt(format_args!("\"Authentication Method\""))
                }
                PropertyType::AuthData => {
                    f.write_fmt(format_args!("\"Authentication Data\""))
                }
                PropertyType::ReqProblemInfo => {
                    f.write_fmt(format_args!("\"Request Problem Information\""))
                }
                PropertyType::WillDelay => {
                    f.write_fmt(format_args!("\"Will Delay Interval\""))
                }
                PropertyType::ReqRespInfo => {
                    f.write_fmt(format_args!("\"Request Response Information\""))
                }
                PropertyType::RespInfo => {
                    f.write_fmt(format_args!("\"Response Information\""))
                }
                PropertyType::ServerReference => {
                    f.write_fmt(format_args!("\"Server Reference\""))
                }
                PropertyType::ReasonString => {
                    f.write_fmt(format_args!("\"Reason String\""))
                }
                PropertyType::RecvMax => f.write_fmt(format_args!("\"Receive Maximum\"")),
                PropertyType::TopicAliasMax => {
                    f.write_fmt(format_args!("\"Topic Alias Maximum\""))
                }
                PropertyType::TopicAlias => f.write_fmt(format_args!("\"Topic Alias\"")),
                PropertyType::MaxQoS => f.write_fmt(format_args!("\"Maximum QoS\"")),
                PropertyType::RetainAvail => {
                    f.write_fmt(format_args!("\"Retain Available\""))
                }
                PropertyType::UserProperty => {
                    f.write_fmt(format_args!("\"User Property\""))
                }
                PropertyType::MaxPacketSize => {
                    f.write_fmt(format_args!("\"Maximum Packet Size\""))
                }
                PropertyType::WildcardSubAvail => {
                    f.write_fmt(format_args!("\"Wildcard Substitution Available\""))
                }
                PropertyType::SubIdAvail => {
                    f.write_fmt(format_args!("\"Subscription Identifier Available\""))
                }
                PropertyType::ShardSubAvail => {
                    f.write_fmt(format_args!("\"Shared Subscription Available\""))
                }
            }
        }
    }
    impl TryFrom<u8> for PropertyType {
        type Error = MqttCodecError;
        fn try_from(value: u8) -> Result<Self, Self::Error> {
            match value {
                0x01 => Ok(PropertyType::PayloadFormat),
                0x02 => Ok(PropertyType::MessageExpiry),
                0x03 => Ok(PropertyType::ContentType),
                0x08 => Ok(PropertyType::ResponseTopic),
                0x09 => Ok(PropertyType::CorrelationData),
                0x0b => Ok(PropertyType::SubscriptionIdentifier),
                0x11 => Ok(PropertyType::SessionExpiryInterval),
                0x12 => Ok(PropertyType::AssignedClientId),
                0x13 => Ok(PropertyType::KeepAlive),
                0x15 => Ok(PropertyType::AuthMethod),
                0x16 => Ok(PropertyType::AuthData),
                0x17 => Ok(PropertyType::ReqProblemInfo),
                0x18 => Ok(PropertyType::WillDelay),
                0x19 => Ok(PropertyType::ReqRespInfo),
                0x1a => Ok(PropertyType::RespInfo),
                0x1c => Ok(PropertyType::ServerReference),
                0x1f => Ok(PropertyType::ReasonString),
                0x21 => Ok(PropertyType::RecvMax),
                0x22 => Ok(PropertyType::TopicAliasMax),
                0x23 => Ok(PropertyType::TopicAlias),
                0x24 => Ok(PropertyType::MaxQoS),
                0x25 => Ok(PropertyType::RetainAvail),
                0x26 => Ok(PropertyType::UserProperty),
                0x27 => Ok(PropertyType::MaxPacketSize),
                0x28 => Ok(PropertyType::WildcardSubAvail),
                0x29 => Ok(PropertyType::SubIdAvail),
                0x2a => Ok(PropertyType::ShardSubAvail),
                p => {
                    Err(
                        MqttCodecError::new(
                            &::alloc::__export::must_use({
                                ::alloc::fmt::format(
                                    format_args!(
                                        "MQTTv5 2.2.2.2 invalid property type identifier: {0}",
                                        p,
                                    ),
                                )
                            }),
                        ),
                    )
                }
            }
        }
    }
    pub struct UserProperty(HashMap<String, Vec<String>>);
    #[automatically_derived]
    impl ::core::fmt::Debug for UserProperty {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::debug_tuple_field1_finish(
                f,
                "UserProperty",
                &&self.0,
            )
        }
    }
    #[automatically_derived]
    impl ::core::default::Default for UserProperty {
        #[inline]
        fn default() -> UserProperty {
            UserProperty(::core::default::Default::default())
        }
    }
    #[automatically_derived]
    impl ::core::clone::Clone for UserProperty {
        #[inline]
        fn clone(&self) -> UserProperty {
            UserProperty(::core::clone::Clone::clone(&self.0))
        }
    }
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for UserProperty {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for UserProperty {
        #[inline]
        fn eq(&self, other: &UserProperty) -> bool {
            self.0 == other.0
        }
    }
    #[automatically_derived]
    impl ::core::cmp::Eq for UserProperty {
        #[inline]
        #[doc(hidden)]
        #[coverage(off)]
        fn assert_receiver_is_total_eq(&self) -> () {
            let _: ::core::cmp::AssertParamIsEq<HashMap<String, Vec<String>>>;
        }
    }
    impl codec::CodecSize for UserProperty {
        fn codec_size(&self) -> u32 {
            let mut size = 0;
            for (key, values) in &self.0 {
                for value in values {
                    size += 1;
                    size += 2 + key.len() as u32;
                    size += 2 + value.len() as u32;
                }
            }
            size
        }
    }
    impl codec::Encode for UserProperty {
        fn encode(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
            for (key, values) in &self.0 {
                for value in values {
                    dest.put_u8(PropertyType::UserProperty as u8);
                    encode_string(key, dest)?;
                    encode_string(value, dest)?;
                }
            }
            Ok(())
        }
    }
    impl codec::Decode for UserProperty {
        /// Decode a single user property key-value pair and add it to the map.
        fn decode(&mut self, src: &mut BytesMut) -> Result<u32, MqttCodecError> {
            let (key_value, key_len) = codec::decode_string(src)?;
            let (value, len) = codec::decode_string(src)?;
            let bytes_read = 2 + key_len + 2 + len;
            self.0.entry(key_value).or_insert_with(Vec::new).push(value);
            Ok(bytes_read)
        }
    }
    impl UserProperty {
        pub fn new() -> Self {
            UserProperty(HashMap::new())
        }
        pub fn clear(&mut self) {
            self.0.clear();
        }
        pub fn clear_codec(&mut self, key: &str) {
            self.0.remove(key);
        }
        pub fn add(&mut self, key: String, value: String) {
            self.0.entry(key).or_insert_with(Vec::new).push(value);
        }
        pub fn get(&self, key: &str) -> Option<&Vec<String>> {
            self.0.get(key)
        }
        pub fn iter(&self) -> impl Iterator<Item = (&String, &Vec<String>)> {
            self.0.iter()
        }
        pub fn is_empty(&self) -> bool {
            self.0.is_empty()
        }
        pub fn len(&self) -> usize {
            self.0.len()
        }
    }
}
pub mod publish {
    use crate::{
        codec, property::UserProperty, MqttCodecError, PacketType, PropertyType, QoSLevel,
    };
    use bytes::{Buf, BufMut, BytesMut};
    use vaux_macro::packet;
    #[repr(u8)]
    pub enum PayloadFormat {
        #[default]
        Bin = 0x00,
        Utf8 = 0x01,
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for PayloadFormat {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::write_str(
                f,
                match self {
                    PayloadFormat::Bin => "Bin",
                    PayloadFormat::Utf8 => "Utf8",
                },
            )
        }
    }
    #[automatically_derived]
    impl ::core::default::Default for PayloadFormat {
        #[inline]
        fn default() -> PayloadFormat {
            Self::Bin
        }
    }
    #[automatically_derived]
    impl ::core::clone::Clone for PayloadFormat {
        #[inline]
        fn clone(&self) -> PayloadFormat {
            *self
        }
    }
    #[automatically_derived]
    impl ::core::marker::Copy for PayloadFormat {}
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for PayloadFormat {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for PayloadFormat {
        #[inline]
        fn eq(&self, other: &PayloadFormat) -> bool {
            let __self_discr = ::core::intrinsics::discriminant_value(self);
            let __arg1_discr = ::core::intrinsics::discriminant_value(other);
            __self_discr == __arg1_discr
        }
    }
    #[automatically_derived]
    impl ::core::cmp::Eq for PayloadFormat {
        #[inline]
        #[doc(hidden)]
        #[coverage(off)]
        fn assert_receiver_is_total_eq(&self) -> () {}
    }
    #[automatically_derived]
    impl ::core::hash::Hash for PayloadFormat {
        #[inline]
        fn hash<__H: ::core::hash::Hasher>(&self, state: &mut __H) -> () {
            let __self_discr = ::core::intrinsics::discriminant_value(self);
            ::core::hash::Hash::hash(&__self_discr, state)
        }
    }
    impl TryFrom<u8> for PayloadFormat {
        type Error = MqttCodecError;
        fn try_from(value: u8) -> Result<Self, Self::Error> {
            match value {
                0x00 => Ok(PayloadFormat::Bin),
                0x01 => Ok(PayloadFormat::Utf8),
                _ => Err(MqttCodecError::new("invalid payload format")),
            }
        }
    }
    impl codec::Encode for PayloadFormat {
        fn encode(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
            dest.put_u8(*self as u8);
            Ok(())
        }
    }
    impl codec::Decode for PayloadFormat {
        fn decode(&mut self, src: &mut BytesMut) -> Result<u32, MqttCodecError> {
            *self = PayloadFormat::try_from(src.get_u8())?;
            Ok(1)
        }
    }
    impl codec::CodecSize for PayloadFormat {
        fn codec_size(&self) -> u32 {
            1
        }
    }
    pub struct Publish {
        pub fixed_header: codec::FixedHeader,
        pub topic_name: String,
        packet_id: Option<u16>,
        pub payload_format: Option<PayloadFormat>,
        pub message_expiry: Option<u32>,
        pub topic_alias: Option<u16>,
        pub response_topic: Option<String>,
        pub correlation_data: Vec<u8>,
        pub subscription_identifiers: Option<u32>,
        pub content_type: Option<String>,
        pub user_properties: UserProperty,
        pub payload: Option<Vec<u8>>,
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for Publish {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            let names: &'static _ = &[
                "fixed_header",
                "topic_name",
                "packet_id",
                "payload_format",
                "message_expiry",
                "topic_alias",
                "response_topic",
                "correlation_data",
                "subscription_identifiers",
                "content_type",
                "user_properties",
                "payload",
            ];
            let values: &[&dyn ::core::fmt::Debug] = &[
                &self.fixed_header,
                &self.topic_name,
                &self.packet_id,
                &self.payload_format,
                &self.message_expiry,
                &self.topic_alias,
                &self.response_topic,
                &self.correlation_data,
                &self.subscription_identifiers,
                &self.content_type,
                &self.user_properties,
                &&self.payload,
            ];
            ::core::fmt::Formatter::debug_struct_fields_finish(
                f,
                "Publish",
                names,
                values,
            )
        }
    }
    #[automatically_derived]
    impl ::core::clone::Clone for Publish {
        #[inline]
        fn clone(&self) -> Publish {
            Publish {
                fixed_header: ::core::clone::Clone::clone(&self.fixed_header),
                topic_name: ::core::clone::Clone::clone(&self.topic_name),
                packet_id: ::core::clone::Clone::clone(&self.packet_id),
                payload_format: ::core::clone::Clone::clone(&self.payload_format),
                message_expiry: ::core::clone::Clone::clone(&self.message_expiry),
                topic_alias: ::core::clone::Clone::clone(&self.topic_alias),
                response_topic: ::core::clone::Clone::clone(&self.response_topic),
                correlation_data: ::core::clone::Clone::clone(&self.correlation_data),
                subscription_identifiers: ::core::clone::Clone::clone(
                    &self.subscription_identifiers,
                ),
                content_type: ::core::clone::Clone::clone(&self.content_type),
                user_properties: ::core::clone::Clone::clone(&self.user_properties),
                payload: ::core::clone::Clone::clone(&self.payload),
            }
        }
    }
    #[automatically_derived]
    impl ::core::cmp::Eq for Publish {
        #[inline]
        #[doc(hidden)]
        #[coverage(off)]
        fn assert_receiver_is_total_eq(&self) -> () {
            let _: ::core::cmp::AssertParamIsEq<codec::FixedHeader>;
            let _: ::core::cmp::AssertParamIsEq<String>;
            let _: ::core::cmp::AssertParamIsEq<Option<u16>>;
            let _: ::core::cmp::AssertParamIsEq<Option<PayloadFormat>>;
            let _: ::core::cmp::AssertParamIsEq<Option<u32>>;
            let _: ::core::cmp::AssertParamIsEq<Option<u16>>;
            let _: ::core::cmp::AssertParamIsEq<Option<String>>;
            let _: ::core::cmp::AssertParamIsEq<Vec<u8>>;
            let _: ::core::cmp::AssertParamIsEq<Option<u32>>;
            let _: ::core::cmp::AssertParamIsEq<Option<String>>;
            let _: ::core::cmp::AssertParamIsEq<UserProperty>;
            let _: ::core::cmp::AssertParamIsEq<Option<Vec<u8>>>;
        }
    }
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for Publish {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for Publish {
        #[inline]
        fn eq(&self, other: &Publish) -> bool {
            self.fixed_header == other.fixed_header
                && self.topic_name == other.topic_name
                && self.packet_id == other.packet_id
                && self.payload_format == other.payload_format
                && self.message_expiry == other.message_expiry
                && self.topic_alias == other.topic_alias
                && self.response_topic == other.response_topic
                && self.correlation_data == other.correlation_data
                && self.subscription_identifiers == other.subscription_identifiers
                && self.content_type == other.content_type
                && self.user_properties == other.user_properties
                && self.payload == other.payload
        }
    }
    impl Publish {
        pub fn new_with_fixed_header(
            fixed_header: codec::FixedHeader,
        ) -> Result<Self, codec::MqttCodecError> {
            if fixed_header.packet_type != codec::PacketType::Publish {
                return Err(
                    MqttCodecError::new(
                        ::alloc::__export::must_use({
                                ::alloc::fmt::format(
                                    format_args!(
                                        "Unsuppprted PacketType for {0}",
                                        "#struct_name",
                                    ),
                                )
                            })
                            .as_str(),
                    ),
                );
            }
            Ok(Self {
                fixed_header,
                ..Default::default()
            })
        }
    }
    impl codec::CodecSize for Publish {
        fn codec_size(&self) -> u32 {
            use codec::PropertyCodecSize;
            let mut total_size = 0;
            total_size += 2;
            total_size += self.topic_name.len() as u32;
            if let Some(_) = &self.packet_id {
                total_size += 2;
            }
            total_size += codec::codec_size_opt_vec_u8_raw(&self.payload);
            let property_size = self.property_size();
            total_size + property_size + codec::variable_byte_int_size(property_size)
        }
    }
    impl codec::PropertyCodecSize for Publish {
        fn property_size(&self) -> u32 {
            use codec::CodecSize;
            let mut property_size = 0;
            if let Some(field) = &self.payload_format {
                property_size += field.codec_size();
            }
            if let Some(_) = &self.message_expiry {
                property_size += 1 + 4;
            }
            if let Some(_) = &self.topic_alias {
                property_size += 1 + 2;
            }
            if let Some(field_name) = &self.response_topic {
                let value_size = field_name.len() as u32 + 2;
                property_size += 1 + value_size;
            }
            if !Vec::is_empty(&self.correlation_data) {
                if !self.correlation_data.is_empty() {
                    property_size += 1 + 2 + self.correlation_data.len() as u32;
                }
            }
            property_size
                += 1
                    + codec::codec_size_opt_variable_byte_int_ref(
                        &self.subscription_identifiers,
                    );
            if let Some(field_name) = &self.content_type {
                let value_size = field_name.len() as u32 + 2;
                property_size += 1 + value_size;
            }
            property_size += self.user_properties.codec_size();
            property_size
        }
    }
    impl codec::Encode for Publish {
        fn encode(&self, dest: &mut bytes::BytesMut) -> Result<(), MqttCodecError> {
            use bytes::{BufMut, BytesMut};
            use codec::{CodecSize, PropertyCodecSize};
            self.fixed_header.encode(dest)?;
            codec::encode_variable_byte_int(self.codec_size(), dest)?;
            codec::encode_string(&self.topic_name, dest)?;
            if let Some(v) = self.packet_id {
                dest.put_u16(v);
            }
            codec::encode_variable_byte_int(self.property_size(), dest)?;
            if let Some(payload_format) = self.payload_format.as_ref() {
                dest.put_u8(PropertyType::PayloadFormat as u8);
                payload_format.encode(dest)?;
            }
            if let Some(v) = self.message_expiry {
                dest.put_u8(PropertyType::MessageExpiry as u8);
                dest.put_u32(v);
            }
            if let Some(v) = self.topic_alias {
                dest.put_u8(PropertyType::TopicAlias as u8);
                dest.put_u16(v);
            }
            if let Some(v) = self.response_topic.as_ref() {
                dest.put_u8(PropertyType::ResponseTopic as u8);
                codec::encode_string(v, dest)?;
            }
            if !Vec::is_empty(&self.correlation_data) {
                dest.put_u8(PropertyType::CorrelationData as u8);
                codec::encode_array_field(&self.correlation_data, dest)?;
            }
            if let Some(f) = self.subscription_identifiers.as_ref() {
                dest.put_u8(PropertyType::SubscriptionIdentifier as u8);
                codec::encode_opt_variable_byte_int(
                    self.subscription_identifiers,
                    dest,
                )?;
            }
            if let Some(v) = self.content_type.as_ref() {
                dest.put_u8(PropertyType::ContentType as u8);
                codec::encode_string(v, dest)?;
            }
            self.user_properties.encode(dest)?;
            if let Some(f) = self.payload.as_ref() {
                codec::encode_opt_vec_u8_raw(self.payload, dest)?;
            }
            Ok(())
        }
    }
    impl codec::Decode for Publish {
        fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<u32, MqttCodecError> {
            use bytes::{BufMut, Buf, BytesMut};
            let mut bytes_read = 0;
            bytes_read += self.topic_name.decode(src)?;
            let mut value = u16::default();
            bytes_read += value.decode(src)?;
            self.packet_id = Some(value);
            let (property_length, var_bytes_read) = codec::decode_variable_byte_int(
                src,
            )?;
            bytes_read += var_bytes_read;
            let mut property_bytes_read = 0;
            while property_bytes_read < property_length {
                let property_type = src.get_u8().try_into()?;
                property_bytes_read += 1;
                match property_type {
                    PropertyType::PayloadFormat => {
                        let mut value = PayloadFormat::default();
                        property_bytes_read += value.decode(src)?;
                        self.payload_format = Some(value);
                    }
                    PropertyType::MessageExpiry => {
                        let mut value = u32::default();
                        property_bytes_read += value.decode(src)?;
                        self.message_expiry = Some(value);
                    }
                    PropertyType::TopicAlias => {
                        let mut value = u16::default();
                        property_bytes_read += value.decode(src)?;
                        self.topic_alias = Some(value);
                    }
                    PropertyType::ResponseTopic => {
                        let mut value = String::default();
                        property_bytes_read += value.decode(src)?;
                        self.response_topic = Some(value);
                    }
                    PropertyType::CorrelationData => {
                        let (value, var_bytes_read) = codec::decode_array_field(src)?;
                        bytes_read += var_bytes_read;
                        self.correlation_data = value;
                    }
                    PropertyType::SubscriptionIdentifier => {
                        let (value, decode_bytes_read) = codec::decode_opt_variable_byte_int(
                            src,
                        )?;
                        property_bytes_read += decode_bytes_read;
                        self.subscription_identifiers = value;
                    }
                    PropertyType::ContentType => {
                        let mut value = String::default();
                        property_bytes_read += value.decode(src)?;
                        self.content_type = Some(value);
                    }
                    PropertyType::UserProperty => {
                        property_bytes_read += self.user_properties.decode(src)?;
                    }
                    _ => {
                        return Err(
                            codec::MqttCodecError::new_with_kind(
                                ::alloc::__export::must_use({
                                        ::alloc::fmt::format(
                                            format_args!(
                                                "MQTT v5 property type {0:?} is not supported",
                                                property_type,
                                            ),
                                        )
                                    })
                                    .as_str(),
                                codec::ErrorKind::UnsupportedProperty(property_type as u8),
                            ),
                        );
                    }
                }
            }
            bytes_read += property_bytes_read;
            Ok(bytes_read)
        }
    }
    impl Default for Publish {
        fn default() -> Self {
            Publish {
                fixed_header: codec::FixedHeader::new(PacketType::Publish),
                topic_name: String::new(),
                packet_id: None,
                payload_format: None,
                message_expiry: None,
                topic_alias: None,
                response_topic: None,
                correlation_data: Vec::new(),
                subscription_identifiers: None,
                content_type: None,
                user_properties: UserProperty::default(),
                payload: None,
            }
        }
    }
    impl Publish {
        /// Create a new Publish packet with the given topic name and QoS level and message
        /// string payload. The payload format is set to UTF-8.
        ///
        /// ### Header Flags
        /// * QoS level is set to the given value.
        /// * Duplicate flag is set to false.
        /// * Retain flag is set to false.
        ///
        /// ### Errors
        /// * If `packet_id` is `None` and `qos_level` is not `AtMostOnce`, an error is returned.
        ///   This is because a packet identifier is required for QoS 1 and 2. See MQTTv5 specification
        ///   section 3.3.2.2.
        /// * If `packet_id` is `Some`, not 0, and `qos_level` is `AtMostOnce`, an error is returned.
        ///   This is because a packet identifier is not supported for QoS 0. See MQTTv5 specification
        ///  section 3.3.2.2.
        pub fn new_with_message(
            packet_id: Option<u16>,
            topic: String,
            qos_level: QoSLevel,
            message: &str,
        ) -> Result<Self, MqttCodecError> {
            Publish::new_with_payload(
                    packet_id,
                    topic,
                    qos_level,
                    message.as_bytes().to_vec(),
                )
                .and_then(|p| Ok(p.with_payload_format(PayloadFormat::Utf8)))
        }
        /// Create a new Publish packet with the given topic name and QoS level and
        /// binary payload.
        ///
        /// ### Header Flags
        /// * QoS level is set to the given value.
        /// * Duplicate flag is set to false.
        /// * Retain flag is set to false.
        ///
        /// ### Errors
        /// * If `packet_id` is `None` and `qos_level` is not `AtMostOnce`, an error is returned.
        ///   This is because a packet identifier is required for QoS 1 and 2. See MQTTv5 specification
        ///   section 3.3.2.2.
        /// * If `packet_id` is `Some`, not 0, and `qos_level` is `AtMostOnce`, an error is returned.
        ///   This is because a packet identifier is not supported for QoS 0. See MQTTv5 specification
        ///  section 3.3.2.2.
        pub fn new_with_payload(
            packet_id: Option<u16>,
            topic: String,
            qos_level: QoSLevel,
            payload: Vec<u8>,
        ) -> Result<Self, MqttCodecError> {
            if packet_id.is_none() && qos_level != QoSLevel::AtMostOnce {
                return Err(
                    MqttCodecError::new(
                        "Mqttv5 3.3.2.2 QOS level must be \"At Most Once\" (0) when no packet identifier set",
                    ),
                );
            } else if packet_id.is_some() && qos_level == QoSLevel::AtMostOnce {
                return Err(
                    MqttCodecError::new(
                        "Mqttv5 3.3.2.2 Packet Identifier not supported when QOS level is set to \"At Most Once\" (0)",
                    ),
                );
            }
            Publish {
                fixed_header: codec::FixedHeader::new(PacketType::Publish),
                topic_name: topic,
                packet_id,
                payload: Some(payload),
                ..Default::default()
            }
                .with_qos(qos_level)
                .with_payload_format(PayloadFormat::Bin)
                .with_dup(false)
        }
        pub fn new_from_header(
            fixed_header: codec::FixedHeader,
        ) -> Result<Self, MqttCodecError> {
            match fixed_header.packet_type {
                PacketType::Publish => {
                    Ok(Publish {
                        fixed_header,
                        ..Default::default()
                    })
                }
                p => {
                    Err(MqttCodecError {
                        reason: ::alloc::__export::must_use({
                            ::alloc::fmt::format(
                                format_args!("unable to construct from {0}", p),
                            )
                        }),
                        kind: crate::codec::ErrorKind::MalformedPacket,
                    })
                }
            }
        }
        pub fn qos(&self) -> QoSLevel {
            self.fixed_header.qos()
        }
        pub fn set_qos(&mut self, qos: QoSLevel) {
            self.fixed_header.set_qos(qos);
        }
        pub fn with_qos(mut self, qos: QoSLevel) -> Self {
            self.set_qos(qos);
            self
        }
        pub fn dup(&self) -> bool {
            self.fixed_header.dup()
        }
        pub fn set_dup(&mut self, dup: bool) -> Result<(), MqttCodecError> {
            if self.fixed_header.qos() == QoSLevel::AtMostOnce && dup {
                return Err(
                    MqttCodecError::new(
                        "Mqttv53.3.2.2 QOS level must not be At
    Most Once with DUP true",
                    ),
                );
            }
            self.fixed_header.set_dup(dup);
            Ok(())
        }
        pub fn with_dup(mut self, dup: bool) -> Result<Self, MqttCodecError> {
            self.set_dup(dup)?;
            Ok(self)
        }
        pub fn retain(&self) -> bool {
            self.fixed_header.retain()
        }
        pub fn set_retain(&mut self, retain: bool) {
            self.fixed_header.set_retain(retain);
        }
        pub fn with_retain(mut self, retain: bool) -> Self {
            self.fixed_header.set_retain(retain);
            self
        }
        pub fn packet_id(&self) -> Option<u16> {
            self.packet_id
        }
        pub fn set_packet_id(&mut self, id: Option<u16>) -> Result<(), MqttCodecError> {
            if self.fixed_header.qos() == QoSLevel::AtMostOnce {
                return Err(
                    MqttCodecError::new(
                        "Mqttv53.3.2.2 QOS level must not be At Most Once",
                    ),
                );
            }
            self.packet_id = id;
            Ok(())
        }
        pub fn with_packet_id(
            mut self,
            id: Option<u16>,
        ) -> Result<Self, MqttCodecError> {
            self.set_packet_id(id)?;
            Ok(self)
        }
        pub fn with_topic_alias(mut self, alias: u16) -> Self {
            self.topic_alias = Some(alias);
            self
        }
        pub fn set_payload_format(&mut self, format: PayloadFormat) {
            if format == PayloadFormat::Bin {
                self.payload_format = None;
                return;
            }
            self.payload_format = Some(format);
        }
        pub fn with_payload_format(mut self, format: PayloadFormat) -> Self {
            self.set_payload_format(format);
            self
        }
    }
}
pub mod pubresp {
    use crate::{
        codec::{self, CodecSize, Decode, ErrorKind, PropertyCodecSize},
        property::UserProperty, MqttCodecError, PacketType, PropertyType, Reason,
    };
    use bytes::{Buf, BufMut};
    use vaux_macro::PropertyCodecSize;
    pub enum PubRelCompReason {
        #[default]
        Success = 0x00,
        PacketIdInUse = 0x91,
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for PubRelCompReason {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::write_str(
                f,
                match self {
                    PubRelCompReason::Success => "Success",
                    PubRelCompReason::PacketIdInUse => "PacketIdInUse",
                },
            )
        }
    }
    #[automatically_derived]
    impl ::core::clone::Clone for PubRelCompReason {
        #[inline]
        fn clone(&self) -> PubRelCompReason {
            *self
        }
    }
    #[automatically_derived]
    impl ::core::marker::Copy for PubRelCompReason {}
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for PubRelCompReason {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for PubRelCompReason {
        #[inline]
        fn eq(&self, other: &PubRelCompReason) -> bool {
            let __self_discr = ::core::intrinsics::discriminant_value(self);
            let __arg1_discr = ::core::intrinsics::discriminant_value(other);
            __self_discr == __arg1_discr
        }
    }
    #[automatically_derived]
    impl ::core::cmp::Eq for PubRelCompReason {
        #[inline]
        #[doc(hidden)]
        #[coverage(off)]
        fn assert_receiver_is_total_eq(&self) -> () {}
    }
    #[automatically_derived]
    impl ::core::default::Default for PubRelCompReason {
        #[inline]
        fn default() -> PubRelCompReason {
            Self::Success
        }
    }
    pub type PubAck = PubResp;
    pub type PubRec = PubResp;
    pub type PubComp = PubResp;
    pub type PubRel = PubResp;
    pub struct PubResp {
        fixed_header: codec::FixedHeader,
        pub packet_id: u16,
        reason: Option<codec::Reason>,
        #[codec(property_type = "PropertyType::ReasonString")]
        reason_desc: Option<String>,
        #[codec(property_type = "PropertyType::UserProperty")]
        user_properties: UserProperty,
    }
    #[automatically_derived]
    impl ::core::default::Default for PubResp {
        #[inline]
        fn default() -> PubResp {
            PubResp {
                fixed_header: ::core::default::Default::default(),
                packet_id: ::core::default::Default::default(),
                reason: ::core::default::Default::default(),
                reason_desc: ::core::default::Default::default(),
                user_properties: ::core::default::Default::default(),
            }
        }
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for PubResp {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::debug_struct_field5_finish(
                f,
                "PubResp",
                "fixed_header",
                &self.fixed_header,
                "packet_id",
                &self.packet_id,
                "reason",
                &self.reason,
                "reason_desc",
                &self.reason_desc,
                "user_properties",
                &&self.user_properties,
            )
        }
    }
    #[automatically_derived]
    impl ::core::clone::Clone for PubResp {
        #[inline]
        fn clone(&self) -> PubResp {
            PubResp {
                fixed_header: ::core::clone::Clone::clone(&self.fixed_header),
                packet_id: ::core::clone::Clone::clone(&self.packet_id),
                reason: ::core::clone::Clone::clone(&self.reason),
                reason_desc: ::core::clone::Clone::clone(&self.reason_desc),
                user_properties: ::core::clone::Clone::clone(&self.user_properties),
            }
        }
    }
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for PubResp {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for PubResp {
        #[inline]
        fn eq(&self, other: &PubResp) -> bool {
            self.packet_id == other.packet_id && self.fixed_header == other.fixed_header
                && self.reason == other.reason && self.reason_desc == other.reason_desc
                && self.user_properties == other.user_properties
        }
    }
    #[automatically_derived]
    impl ::core::cmp::Eq for PubResp {
        #[inline]
        #[doc(hidden)]
        #[coverage(off)]
        fn assert_receiver_is_total_eq(&self) -> () {
            let _: ::core::cmp::AssertParamIsEq<codec::FixedHeader>;
            let _: ::core::cmp::AssertParamIsEq<u16>;
            let _: ::core::cmp::AssertParamIsEq<Option<codec::Reason>>;
            let _: ::core::cmp::AssertParamIsEq<Option<String>>;
            let _: ::core::cmp::AssertParamIsEq<UserProperty>;
        }
    }
    impl codec::PropertyCodecSize for PubResp {
        fn property_size(&self) -> u32 {
            use codec::CodecSize;
            let mut property_size = 0;
            if let Some(field_name) = &self.reason_desc {
                let value_size = field_name.len() as u32 + 2;
                property_size += 1 + value_size;
            }
            property_size += self.user_properties.codec_size();
            property_size
        }
    }
    impl codec::CodecSize for PubResp {
        fn codec_size(&self) -> u32 {
            let mut size = 2;
            let property_size = self.property_size();
            if self.reason.unwrap_or_default() == codec::Reason::Success
                && self.property_size() == 0
            {
                return size;
            }
            size += 1;
            size += codec::variable_byte_int_size(property_size);
            if let Some(reason_desc) = &self.reason_desc {
                size += 3 + reason_desc.len() as u32;
            }
            size += self.user_properties.codec_size();
            size
        }
    }
    impl codec::Encode for PubResp {
        fn encode(&self, dest: &mut bytes::BytesMut) -> Result<(), MqttCodecError> {
            self.fixed_header.encode(dest)?;
            if self.reason.unwrap_or_default() == codec::Reason::Success
                && self.property_size() == 0
            {
                dest.put_u8(2);
                dest.put_u16(self.packet_id);
                return Ok(());
            }
            codec::encode_variable_byte_int(self.codec_size(), dest)?;
            dest.put_u16(self.packet_id);
            let reason = self.reason.unwrap_or_default();
            dest.put_u8(reason as u8);
            codec::encode_variable_byte_int(self.property_size(), dest)?;
            if let Some(v) = self.reason_desc.as_ref() {
                dest.put_u8(PropertyType::ReasonString as u8);
                codec::encode_string(v, dest)?;
            }
            self.user_properties.encode(dest)?;
            Ok(())
        }
    }
    impl Decode for PubResp {
        fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<u32, MqttCodecError> {
            if src.remaining() < 2 {
                return Err(
                    MqttCodecError::new_with_kind(
                        "Insufficient data",
                        codec::ErrorKind::InsufficientData(2, src.remaining()),
                    ),
                );
            }
            self.packet_id = src.get_u16();
            if src.remaining() == 0 {
                return Ok(2);
            }
            self.reason = Some(codec::Reason::try_from(src.get_u8())?);
            let property_length = codec::decode_variable_byte_int(src)?;
            let mut read_bytes = 3 + property_length.1;
            if src.remaining() < property_length.0 as usize {
                return Err(
                    MqttCodecError::new_with_kind(
                        "Insufficient data for properties",
                        codec::ErrorKind::InsufficientData(
                            property_length.0 as usize,
                            src.remaining(),
                        ),
                    ),
                );
            }
            let mut properties_bytes = src.split_to(property_length.0 as usize);
            while properties_bytes.has_remaining() {
                let property_type = PropertyType::try_from(properties_bytes.get_u8())?;
                match property_type {
                    PropertyType::ReasonString => {
                        let (value, len) = codec::decode_string(&mut properties_bytes)?;
                        self.reason_desc = Some(value);
                        read_bytes += 1 + len;
                    }
                    PropertyType::UserProperty => {
                        read_bytes
                            += self.user_properties.decode(&mut properties_bytes)?;
                    }
                    _ => {
                        return Err(
                            MqttCodecError::new_with_kind(
                                "Invalid property for PUBREL/PUBCOMP",
                                codec::ErrorKind::InvalidPacket,
                            ),
                        );
                    }
                }
            }
            Ok(read_bytes)
        }
    }
    impl PubResp {
        pub fn new_puback_with_packet_id(packet_id: u16) -> Self {
            PubResp {
                fixed_header: codec::FixedHeader::new(codec::PacketType::PubAck),
                packet_id,
                ..Default::default()
            }
        }
        pub fn new_pubrec_with_packet_id(packet_id: u16) -> Self {
            PubResp {
                fixed_header: codec::FixedHeader::new(codec::PacketType::PubRec),
                packet_id,
                ..Default::default()
            }
        }
        pub fn new_pubrel_with_packet_id(packet_id: u16) -> Self {
            PubResp {
                fixed_header: codec::FixedHeader::new(codec::PacketType::PubRel),
                packet_id,
                ..Default::default()
            }
        }
        pub fn new_pubcomp_with_packet_id(packet_id: u16) -> Self {
            PubResp {
                fixed_header: codec::FixedHeader::new(codec::PacketType::PubComp),
                packet_id,
                ..Default::default()
            }
        }
        pub fn new_with_fixed_header(
            fixed_header: codec::FixedHeader,
        ) -> Result<Self, MqttCodecError> {
            match fixed_header.packet_type {
                codec::PacketType::PubAck
                | codec::PacketType::PubRec
                | codec::PacketType::PubComp
                | codec::PacketType::PubRel => {
                    Ok(PubResp {
                        fixed_header,
                        ..Default::default()
                    })
                }
                _ => {
                    Err(
                        MqttCodecError::new_with_kind(
                            "Packet must be one of [PubAck, PubRec, PubComp, PubRel]",
                            codec::ErrorKind::InvalidPacket,
                        ),
                    )
                }
            }
        }
        pub fn reason(&self) -> Option<Reason> {
            self.reason
        }
        pub fn set_reason(&mut self, reason: Reason) -> Result<(), MqttCodecError> {
            if self.supported_reason(reason) {
                self.reason = Some(reason);
                Ok(())
            } else {
                Err(MqttCodecError {
                    reason: "unsupported reason".to_string(),
                    kind: ErrorKind::UnsupportedReason(reason as u8),
                })
            }
        }
        fn supported_reason(&self, reason: Reason) -> bool {
            match self.fixed_header.packet_type {
                PacketType::PubAck | PacketType::PubRec => {
                    #[allow(non_exhaustive_omitted_patterns)]
                    match reason {
                        Reason::Success
                        | Reason::NoSubscribers
                        | Reason::UnspecifiedErr
                        | Reason::ImplementationErr
                        | Reason::NotAuthorized
                        | Reason::InvalidTopicName
                        | Reason::PacketIdInUse
                        | Reason::QuotaExceeded
                        | Reason::PayloadFormatErr => true,
                        _ => false,
                    }
                }
                PacketType::PubComp | PacketType::PubRel => {
                    #[allow(non_exhaustive_omitted_patterns)]
                    match reason {
                        Reason::Success | Reason::PacketIdInUse => true,
                        _ => false,
                    }
                }
                _ => false,
            }
        }
    }
}
pub mod subscribe {
    use crate::codec::{ErrorKind, MAX_VARIABLE_BYTE_INT, MIN_VARIABLE_BYTE_INT};
    use crate::{codec, MqttCodecError, PropertyType};
    use crate::{property::UserProperty, MqttError, MqttVersion, QoSLevel};
    use bytes::{Buf, BufMut};
    use vaux_macro::{packet, CodecSize, Decode, Encode};
    /// MQTT v5 3.8.3.1 Subscription Options
    /// bits 4 and 5 of the subscription options hold the retain handling flag.
    /// Retain handling is used to determine how messages published with the
    /// retain flag set to ```true``` are handled when the ```SUBSCRIBE``` packet
    /// is received.
    #[repr(u8)]
    pub enum RetainHandling {
        #[default]
        Send,
        SendNew,
        None,
    }
    #[automatically_derived]
    impl ::core::marker::Copy for RetainHandling {}
    #[automatically_derived]
    impl ::core::clone::Clone for RetainHandling {
        #[inline]
        fn clone(&self) -> RetainHandling {
            *self
        }
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for RetainHandling {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::write_str(
                f,
                match self {
                    RetainHandling::Send => "Send",
                    RetainHandling::SendNew => "SendNew",
                    RetainHandling::None => "None",
                },
            )
        }
    }
    #[automatically_derived]
    impl ::core::default::Default for RetainHandling {
        #[inline]
        fn default() -> RetainHandling {
            Self::Send
        }
    }
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for RetainHandling {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for RetainHandling {
        #[inline]
        fn eq(&self, other: &RetainHandling) -> bool {
            let __self_discr = ::core::intrinsics::discriminant_value(self);
            let __arg1_discr = ::core::intrinsics::discriminant_value(other);
            __self_discr == __arg1_discr
        }
    }
    #[automatically_derived]
    impl ::core::cmp::Eq for RetainHandling {
        #[inline]
        #[doc(hidden)]
        #[coverage(off)]
        fn assert_receiver_is_total_eq(&self) -> () {}
    }
    impl TryFrom<u8> for RetainHandling {
        type Error = MqttCodecError;
        fn try_from(value: u8) -> Result<Self, Self::Error> {
            match value {
                0x00 => Ok(RetainHandling::Send),
                0x01 => Ok(RetainHandling::SendNew),
                0x02 => Ok(RetainHandling::None),
                v => {
                    Err(
                        MqttCodecError::new(
                            ::alloc::__export::must_use({
                                    ::alloc::fmt::format(
                                        format_args!("Mqttv5 3.8.3.1 invalid retain option: {0}", v),
                                    )
                                })
                                .as_str(),
                        ),
                    )
                }
            }
        }
    }
    pub struct SubAck {
        pub fixed_header: codec::FixedHeader,
        packet_id: u16,
        pub reason: Option<String>,
        pub user_properties: UserProperty,
        pub reason_codes: Vec<u8>,
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for SubAck {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::debug_struct_field5_finish(
                f,
                "SubAck",
                "fixed_header",
                &self.fixed_header,
                "packet_id",
                &self.packet_id,
                "reason",
                &self.reason,
                "user_properties",
                &self.user_properties,
                "reason_codes",
                &&self.reason_codes,
            )
        }
    }
    #[automatically_derived]
    impl ::core::default::Default for SubAck {
        #[inline]
        fn default() -> SubAck {
            SubAck {
                fixed_header: ::core::default::Default::default(),
                packet_id: ::core::default::Default::default(),
                reason: ::core::default::Default::default(),
                user_properties: ::core::default::Default::default(),
                reason_codes: ::core::default::Default::default(),
            }
        }
    }
    #[automatically_derived]
    impl ::core::clone::Clone for SubAck {
        #[inline]
        fn clone(&self) -> SubAck {
            SubAck {
                fixed_header: ::core::clone::Clone::clone(&self.fixed_header),
                packet_id: ::core::clone::Clone::clone(&self.packet_id),
                reason: ::core::clone::Clone::clone(&self.reason),
                user_properties: ::core::clone::Clone::clone(&self.user_properties),
                reason_codes: ::core::clone::Clone::clone(&self.reason_codes),
            }
        }
    }
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for SubAck {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for SubAck {
        #[inline]
        fn eq(&self, other: &SubAck) -> bool {
            self.packet_id == other.packet_id && self.fixed_header == other.fixed_header
                && self.reason == other.reason
                && self.user_properties == other.user_properties
                && self.reason_codes == other.reason_codes
        }
    }
    #[automatically_derived]
    impl ::core::cmp::Eq for SubAck {
        #[inline]
        #[doc(hidden)]
        #[coverage(off)]
        fn assert_receiver_is_total_eq(&self) -> () {
            let _: ::core::cmp::AssertParamIsEq<codec::FixedHeader>;
            let _: ::core::cmp::AssertParamIsEq<u16>;
            let _: ::core::cmp::AssertParamIsEq<Option<String>>;
            let _: ::core::cmp::AssertParamIsEq<UserProperty>;
            let _: ::core::cmp::AssertParamIsEq<Vec<u8>>;
        }
    }
    impl SubAck {
        pub fn new_with_fixed_header(
            fixed_header: codec::FixedHeader,
        ) -> Result<Self, codec::MqttCodecError> {
            if fixed_header.packet_type != codec::PacketType::SubAck {
                return Err(
                    MqttCodecError::new(
                        ::alloc::__export::must_use({
                                ::alloc::fmt::format(
                                    format_args!(
                                        "Unsuppprted PacketType for {0}",
                                        "#struct_name",
                                    ),
                                )
                            })
                            .as_str(),
                    ),
                );
            }
            Ok(Self {
                fixed_header,
                ..Default::default()
            })
        }
    }
    impl codec::CodecSize for SubAck {
        fn codec_size(&self) -> u32 {
            use codec::PropertyCodecSize;
            let mut total_size = 0;
            total_size += 2;
            total_size += codec::codec_size_vec_u8_raw(&self.reason_codes);
            let property_size = self.property_size();
            total_size + property_size + codec::variable_byte_int_size(property_size)
        }
    }
    impl codec::PropertyCodecSize for SubAck {
        fn property_size(&self) -> u32 {
            use codec::CodecSize;
            let mut property_size = 0;
            if let Some(field_name) = &self.reason {
                let value_size = field_name.len() as u32 + 2;
                property_size += 1 + value_size;
            }
            property_size += self.user_properties.codec_size();
            property_size
        }
    }
    impl codec::Encode for SubAck {
        fn encode(&self, dest: &mut bytes::BytesMut) -> Result<(), MqttCodecError> {
            use bytes::{BufMut, BytesMut};
            use codec::{CodecSize, PropertyCodecSize};
            self.fixed_header.encode(dest)?;
            codec::encode_variable_byte_int(self.codec_size(), dest)?;
            dest.put_u16(self.packet_id);
            codec::encode_variable_byte_int(self.property_size(), dest)?;
            if let Some(v) = self.reason.as_ref() {
                dest.put_u8(PropertyType::ReasonString as u8);
                codec::encode_string(v, dest)?;
            }
            self.user_properties.encode(dest)?;
            encode_suback_reason_vec(&self.reason_codes, dest)?;
            Ok(())
        }
    }
    impl codec::Decode for SubAck {
        fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<u32, MqttCodecError> {
            use bytes::{BufMut, Buf, BytesMut};
            let mut bytes_read = 0;
            self.packet_id = src.get_u16();
            bytes_read += 2;
            let (property_length, var_bytes_read) = codec::decode_variable_byte_int(
                src,
            )?;
            bytes_read += var_bytes_read;
            let mut property_bytes_read = 0;
            while property_bytes_read < property_length {
                let property_type = src.get_u8().try_into()?;
                property_bytes_read += 1;
                match property_type {
                    PropertyType::ReasonString => {
                        let mut value = String::default();
                        property_bytes_read += value.decode(src)?;
                        self.reason = Some(value);
                    }
                    PropertyType::UserProperty => {
                        property_bytes_read += self.user_properties.decode(src)?;
                    }
                    _ => {
                        return Err(
                            codec::MqttCodecError::new_with_kind(
                                ::alloc::__export::must_use({
                                        ::alloc::fmt::format(
                                            format_args!(
                                                "MQTT v5 property type {0:?} is not supported",
                                                property_type,
                                            ),
                                        )
                                    })
                                    .as_str(),
                                codec::ErrorKind::UnsupportedProperty(property_type as u8),
                            ),
                        );
                    }
                }
            }
            bytes_read += property_bytes_read;
            Ok(bytes_read)
        }
    }
    pub fn encode_suback_reason_vec(
        reasons: &Vec<u8>,
        dest: &mut bytes::BytesMut,
    ) -> Result<(), MqttCodecError> {
        dest.put_slice(reasons);
        Ok(())
    }
    pub fn decode_suback_reason_vec(
        reasons: &mut Vec<u8>,
        src: &mut bytes::BytesMut,
    ) -> Result<u32, MqttCodecError> {
        let len = src.remaining();
        reasons.extend_from_slice(&src[..]);
        src.advance(len);
        Ok(len as u32)
    }
    impl SubAck {
        pub fn new_with_packet_id(packet_id: u16) -> Result<Self, MqttCodecError> {
            if packet_id == 0 {
                return Err(
                    MqttCodecError::new_with_kind(
                        "2.2.1 Packet identified must not be 0",
                        ErrorKind::InvalidPacketIdentifier,
                    ),
                );
            }
            Ok(Self {
                packet_id,
                ..Default::default()
            })
        }
        pub fn packet_id(&self) -> u16 {
            self.packet_id
        }
        pub fn set_packet_id(&mut self, packet_id: u16) -> Result<(), MqttCodecError> {
            if packet_id == 0 {
                return Err(
                    MqttCodecError::new_with_kind(
                        "2.2.1 Packet identified must not be 0",
                        ErrorKind::InvalidPacketIdentifier,
                    ),
                );
            }
            self.packet_id = packet_id;
            Ok(())
        }
    }
    /// Subscription represents an MQTT v5 3.8.3 SUBSCRIBE Payload. The Mqtt v5
    /// 3.8.3.1 options are represented as individual fields in the struct.
    pub struct SubscriptionFilter {
        pub filter: String,
        options: u8,
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for SubscriptionFilter {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::debug_struct_field2_finish(
                f,
                "SubscriptionFilter",
                "filter",
                &self.filter,
                "options",
                &&self.options,
            )
        }
    }
    #[automatically_derived]
    impl ::core::default::Default for SubscriptionFilter {
        #[inline]
        fn default() -> SubscriptionFilter {
            SubscriptionFilter {
                filter: ::core::default::Default::default(),
                options: ::core::default::Default::default(),
            }
        }
    }
    #[automatically_derived]
    impl ::core::clone::Clone for SubscriptionFilter {
        #[inline]
        fn clone(&self) -> SubscriptionFilter {
            SubscriptionFilter {
                filter: ::core::clone::Clone::clone(&self.filter),
                options: ::core::clone::Clone::clone(&self.options),
            }
        }
    }
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for SubscriptionFilter {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for SubscriptionFilter {
        #[inline]
        fn eq(&self, other: &SubscriptionFilter) -> bool {
            self.options == other.options && self.filter == other.filter
        }
    }
    #[automatically_derived]
    impl ::core::cmp::Eq for SubscriptionFilter {
        #[inline]
        #[doc(hidden)]
        #[coverage(off)]
        fn assert_receiver_is_total_eq(&self) -> () {
            let _: ::core::cmp::AssertParamIsEq<String>;
            let _: ::core::cmp::AssertParamIsEq<u8>;
        }
    }
    impl codec::Encode for SubscriptionFilter {
        fn encode(&self, dest: &mut bytes::BytesMut) -> Result<(), MqttCodecError> {
            use bytes::{BufMut, BytesMut};
            codec::encode_string(&self.filter, dest)?;
            dest.put_u8(self.options);
            Ok(())
        }
    }
    impl codec::Decode for SubscriptionFilter {
        fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<u32, MqttCodecError> {
            use bytes::{BufMut, Buf, BytesMut};
            let mut bytes_read = 0;
            bytes_read += self.filter.decode(src)?;
            self.options = src.get_u8();
            bytes_read += 1;
            Ok(bytes_read)
        }
    }
    impl codec::CodecSize for SubscriptionFilter {
        fn codec_size(&self) -> u32 {
            let mut total_size = 0;
            total_size += 2;
            total_size += self.filter.len() as u32;
            total_size += 1;
            total_size
        }
    }
    impl SubscriptionFilter {
        /// Create a new Subscription with the given filter and QoSLevel.
        /// The remaining fields are set to their default values.
        pub fn new(filter: String, qos: QoSLevel) -> Self {
            let mut s = Self { filter, options: 0_u8 };
            s.set_qos(qos);
            s
        }
        pub fn qos(&self) -> QoSLevel {
            (self.options & 0b0000_0011).try_into().unwrap_or(QoSLevel::AtMostOnce)
        }
        pub fn with_qos(mut self, qos: QoSLevel) -> Self {
            self.set_qos(qos);
            self
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
            (self.options & 0b0000_1000).try_into().unwrap_or(RetainHandling::Send)
        }
        pub fn set_retain_handling(&mut self, retain: RetainHandling) {
            self.options = (self.options & !0b0000_1000) | ((retain as u8) << 3);
        }
    }
    pub struct Subscribe {
        pub fixed_header: codec::FixedHeader,
        pub packet_id: u16,
        pub subscription_id: Option<u32>,
        pub props: UserProperty,
        pub filter: Vec<SubscriptionFilter>,
    }
    #[automatically_derived]
    impl ::core::default::Default for Subscribe {
        #[inline]
        fn default() -> Subscribe {
            Subscribe {
                fixed_header: ::core::default::Default::default(),
                packet_id: ::core::default::Default::default(),
                subscription_id: ::core::default::Default::default(),
                props: ::core::default::Default::default(),
                filter: ::core::default::Default::default(),
            }
        }
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for Subscribe {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::debug_struct_field5_finish(
                f,
                "Subscribe",
                "fixed_header",
                &self.fixed_header,
                "packet_id",
                &self.packet_id,
                "subscription_id",
                &self.subscription_id,
                "props",
                &self.props,
                "filter",
                &&self.filter,
            )
        }
    }
    #[automatically_derived]
    impl ::core::clone::Clone for Subscribe {
        #[inline]
        fn clone(&self) -> Subscribe {
            Subscribe {
                fixed_header: ::core::clone::Clone::clone(&self.fixed_header),
                packet_id: ::core::clone::Clone::clone(&self.packet_id),
                subscription_id: ::core::clone::Clone::clone(&self.subscription_id),
                props: ::core::clone::Clone::clone(&self.props),
                filter: ::core::clone::Clone::clone(&self.filter),
            }
        }
    }
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for Subscribe {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for Subscribe {
        #[inline]
        fn eq(&self, other: &Subscribe) -> bool {
            self.packet_id == other.packet_id && self.fixed_header == other.fixed_header
                && self.subscription_id == other.subscription_id
                && self.props == other.props && self.filter == other.filter
        }
    }
    #[automatically_derived]
    impl ::core::cmp::Eq for Subscribe {
        #[inline]
        #[doc(hidden)]
        #[coverage(off)]
        fn assert_receiver_is_total_eq(&self) -> () {
            let _: ::core::cmp::AssertParamIsEq<codec::FixedHeader>;
            let _: ::core::cmp::AssertParamIsEq<u16>;
            let _: ::core::cmp::AssertParamIsEq<Option<u32>>;
            let _: ::core::cmp::AssertParamIsEq<UserProperty>;
            let _: ::core::cmp::AssertParamIsEq<Vec<SubscriptionFilter>>;
        }
    }
    impl Subscribe {
        pub fn new_with_fixed_header(
            fixed_header: codec::FixedHeader,
        ) -> Result<Self, codec::MqttCodecError> {
            if fixed_header.packet_type != codec::PacketType::Subscribe {
                return Err(
                    MqttCodecError::new(
                        ::alloc::__export::must_use({
                                ::alloc::fmt::format(
                                    format_args!(
                                        "Unsuppprted PacketType for {0}",
                                        "#struct_name",
                                    ),
                                )
                            })
                            .as_str(),
                    ),
                );
            }
            Ok(Self {
                fixed_header,
                ..Default::default()
            })
        }
    }
    impl codec::CodecSize for Subscribe {
        fn codec_size(&self) -> u32 {
            use codec::PropertyCodecSize;
            let mut total_size = 0;
            total_size += 2;
            if !self.filter.is_empty() {
                total_size
                    += self.filter.iter().map(|item| item.codec_size()).sum::<u32>();
            }
            let property_size = self.property_size();
            total_size + property_size + codec::variable_byte_int_size(property_size)
        }
    }
    impl codec::PropertyCodecSize for Subscribe {
        fn property_size(&self) -> u32 {
            use codec::CodecSize;
            let mut property_size = 0;
            property_size
                += 1
                    + codec::codec_size_opt_variable_byte_int_ref(&self.subscription_id);
            property_size += self.props.codec_size();
            property_size
        }
    }
    impl codec::Encode for Subscribe {
        fn encode(&self, dest: &mut bytes::BytesMut) -> Result<(), MqttCodecError> {
            use bytes::{BufMut, BytesMut};
            use codec::{CodecSize, PropertyCodecSize};
            self.fixed_header.encode(dest)?;
            codec::encode_variable_byte_int(self.codec_size(), dest)?;
            dest.put_u16(self.packet_id);
            codec::encode_variable_byte_int(self.property_size(), dest)?;
            if let Some(f) = self.subscription_id.as_ref() {
                dest.put_u8(PropertyType::SubscriptionIdentifier as u8);
                codec::encode_opt_variable_byte_int(self.subscription_id, dest)?;
            }
            self.props.encode(dest)?;
            for item in self.filter.iter() {
                item.encode(dest)?;
            }
            Ok(())
        }
    }
    impl codec::Decode for Subscribe {
        fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<u32, MqttCodecError> {
            use bytes::{BufMut, Buf, BytesMut};
            let mut bytes_read = 0;
            self.packet_id = src.get_u16();
            bytes_read += 2;
            let (property_length, var_bytes_read) = codec::decode_variable_byte_int(
                src,
            )?;
            bytes_read += var_bytes_read;
            let mut property_bytes_read = 0;
            while property_bytes_read < property_length {
                let property_type = src.get_u8().try_into()?;
                property_bytes_read += 1;
                match property_type {
                    PropertyType::SubscriptionIdentifier => {
                        let (value, decode_bytes_read) = codec::decode_opt_variable_byte_int(
                            src,
                        )?;
                        property_bytes_read += decode_bytes_read;
                        self.subscription_id = value;
                    }
                    PropertyType::UserProperty => {
                        property_bytes_read += self.props.decode(src)?;
                    }
                    _ => {
                        return Err(
                            codec::MqttCodecError::new_with_kind(
                                ::alloc::__export::must_use({
                                        ::alloc::fmt::format(
                                            format_args!(
                                                "MQTT v5 property type {0:?} is not supported",
                                                property_type,
                                            ),
                                        )
                                    })
                                    .as_str(),
                                codec::ErrorKind::UnsupportedProperty(property_type as u8),
                            ),
                        );
                    }
                }
            }
            bytes_read += property_bytes_read;
            Ok(bytes_read)
        }
    }
    impl Subscribe {
        pub fn new_with_packet_id(packet_id: u16) -> Self {
            Self {
                packet_id,
                ..Default::default()
            }
        }
        pub fn new_with_filter(packet_id: u16, filter: Vec<SubscriptionFilter>) -> Self {
            Self {
                packet_id,
                filter,
                ..Default::default()
            }
        }
        pub fn set_subscription_id(&mut self, id: u32) -> Result<(), MqttError> {
            if !(MIN_VARIABLE_BYTE_INT..=MAX_VARIABLE_BYTE_INT).contains(&id) {
                return Err(
                    MqttError::new_from_spec(
                        MqttVersion::V5,
                        "3.8.3",
                        "subscription ID must be between 1 and 268,435,455",
                    ),
                );
            }
            self.subscription_id = Some(id);
            Ok(())
        }
        pub fn add_filter(&mut self, filter: SubscriptionFilter) {
            self.filter.push(filter);
        }
        pub fn remove_filter_at(&mut self, index: usize) -> Option<SubscriptionFilter> {
            if index < self.filter.len() {
                Some(self.filter.remove(index))
            } else {
                None
            }
        }
        pub fn remove_filter_with_topic(
            &mut self,
            topic: &str,
        ) -> Option<SubscriptionFilter> {
            if let Some(pos) = self.filter.iter().position(|f| f.filter == topic) {
                Some(self.filter.remove(pos))
            } else {
                None
            }
        }
        pub fn clear_filters(&mut self) {
            self.filter.clear();
        }
    }
}
pub mod unsubscribe {
    use std::{collections::HashSet, sync::LazyLock};
    use bytes::{Buf, BufMut};
    use vaux_macro::packet;
    use crate::{
        FixedHeader, PacketType, PropertyType, Reason,
        codec::{self, CodecSize, Decode, Encode, MqttCodecError},
        property::UserProperty,
    };
    pub struct UnsubAck {
        pub fixed_header: codec::FixedHeader,
        pub packet_id: u16,
        pub reason: String,
        pub user_properties: UserProperty,
        pub reason_code: Vec<Reason>,
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for UnsubAck {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::debug_struct_field5_finish(
                f,
                "UnsubAck",
                "fixed_header",
                &self.fixed_header,
                "packet_id",
                &self.packet_id,
                "reason",
                &self.reason,
                "user_properties",
                &self.user_properties,
                "reason_code",
                &&self.reason_code,
            )
        }
    }
    #[automatically_derived]
    impl ::core::default::Default for UnsubAck {
        #[inline]
        fn default() -> UnsubAck {
            UnsubAck {
                fixed_header: ::core::default::Default::default(),
                packet_id: ::core::default::Default::default(),
                reason: ::core::default::Default::default(),
                user_properties: ::core::default::Default::default(),
                reason_code: ::core::default::Default::default(),
            }
        }
    }
    #[automatically_derived]
    impl ::core::clone::Clone for UnsubAck {
        #[inline]
        fn clone(&self) -> UnsubAck {
            UnsubAck {
                fixed_header: ::core::clone::Clone::clone(&self.fixed_header),
                packet_id: ::core::clone::Clone::clone(&self.packet_id),
                reason: ::core::clone::Clone::clone(&self.reason),
                user_properties: ::core::clone::Clone::clone(&self.user_properties),
                reason_code: ::core::clone::Clone::clone(&self.reason_code),
            }
        }
    }
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for UnsubAck {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for UnsubAck {
        #[inline]
        fn eq(&self, other: &UnsubAck) -> bool {
            self.packet_id == other.packet_id && self.fixed_header == other.fixed_header
                && self.reason == other.reason
                && self.user_properties == other.user_properties
                && self.reason_code == other.reason_code
        }
    }
    #[automatically_derived]
    impl ::core::cmp::Eq for UnsubAck {
        #[inline]
        #[doc(hidden)]
        #[coverage(off)]
        fn assert_receiver_is_total_eq(&self) -> () {
            let _: ::core::cmp::AssertParamIsEq<codec::FixedHeader>;
            let _: ::core::cmp::AssertParamIsEq<u16>;
            let _: ::core::cmp::AssertParamIsEq<String>;
            let _: ::core::cmp::AssertParamIsEq<UserProperty>;
            let _: ::core::cmp::AssertParamIsEq<Vec<Reason>>;
        }
    }
    impl UnsubAck {
        pub fn new_with_fixed_header(
            fixed_header: codec::FixedHeader,
        ) -> Result<Self, codec::MqttCodecError> {
            if fixed_header.packet_type != codec::PacketType::UnsubAck {
                return Err(
                    MqttCodecError::new(
                        ::alloc::__export::must_use({
                                ::alloc::fmt::format(
                                    format_args!(
                                        "Unsuppprted PacketType for {0}",
                                        "#struct_name",
                                    ),
                                )
                            })
                            .as_str(),
                    ),
                );
            }
            Ok(Self {
                fixed_header,
                ..Default::default()
            })
        }
    }
    impl codec::CodecSize for UnsubAck {
        fn codec_size(&self) -> u32 {
            use codec::PropertyCodecSize;
            let mut total_size = 0;
            total_size += 2;
            if !self.reason_code.is_empty() {
                total_size
                    += self
                        .reason_code
                        .iter()
                        .map(|item| item.codec_size())
                        .sum::<u32>();
            }
            let property_size = self.property_size();
            total_size + property_size + codec::variable_byte_int_size(property_size)
        }
    }
    impl codec::PropertyCodecSize for UnsubAck {
        fn property_size(&self) -> u32 {
            use codec::CodecSize;
            let mut property_size = 0;
            let value_size = self.reason.len() as u32 + 2;
            property_size += 1 + value_size;
            property_size += self.user_properties.codec_size();
            property_size
        }
    }
    impl codec::Encode for UnsubAck {
        fn encode(&self, dest: &mut bytes::BytesMut) -> Result<(), MqttCodecError> {
            use bytes::{BufMut, BytesMut};
            use codec::{CodecSize, PropertyCodecSize};
            self.fixed_header.encode(dest)?;
            codec::encode_variable_byte_int(self.codec_size(), dest)?;
            dest.put_u16(self.packet_id);
            codec::encode_variable_byte_int(self.property_size(), dest)?;
            dest.put_u8(PropertyType::ReasonString as u8);
            codec::encode_string(&self.reason, dest)?;
            self.user_properties.encode(dest)?;
            for item in self.reason_code.iter() {
                item.encode(dest)?;
            }
            Ok(())
        }
    }
    impl codec::Decode for UnsubAck {
        fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<u32, MqttCodecError> {
            use bytes::{BufMut, Buf, BytesMut};
            let mut bytes_read = 0;
            self.packet_id = src.get_u16();
            bytes_read += 2;
            let (property_length, var_bytes_read) = codec::decode_variable_byte_int(
                src,
            )?;
            bytes_read += var_bytes_read;
            let mut property_bytes_read = 0;
            while property_bytes_read < property_length {
                let property_type = src.get_u8().try_into()?;
                property_bytes_read += 1;
                match property_type {
                    PropertyType::ReasonString => {
                        property_bytes_read += self.reason.decode(src)?;
                    }
                    PropertyType::UserProperty => {
                        property_bytes_read += self.user_properties.decode(src)?;
                    }
                    _ => {
                        return Err(
                            codec::MqttCodecError::new_with_kind(
                                ::alloc::__export::must_use({
                                        ::alloc::fmt::format(
                                            format_args!(
                                                "MQTT v5 property type {0:?} is not supported",
                                                property_type,
                                            ),
                                        )
                                    })
                                    .as_str(),
                                codec::ErrorKind::UnsupportedProperty(property_type as u8),
                            ),
                        );
                    }
                }
            }
            bytes_read += property_bytes_read;
            Ok(bytes_read)
        }
    }
    pub struct Unsubscribe {
        pub fixed_header: codec::FixedHeader,
        pub packet_id: u16,
        pub props: UserProperty,
        pub topics: Vec<String>,
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for Unsubscribe {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::debug_struct_field4_finish(
                f,
                "Unsubscribe",
                "fixed_header",
                &self.fixed_header,
                "packet_id",
                &self.packet_id,
                "props",
                &self.props,
                "topics",
                &&self.topics,
            )
        }
    }
    #[automatically_derived]
    impl ::core::default::Default for Unsubscribe {
        #[inline]
        fn default() -> Unsubscribe {
            Unsubscribe {
                fixed_header: ::core::default::Default::default(),
                packet_id: ::core::default::Default::default(),
                props: ::core::default::Default::default(),
                topics: ::core::default::Default::default(),
            }
        }
    }
    #[automatically_derived]
    impl ::core::clone::Clone for Unsubscribe {
        #[inline]
        fn clone(&self) -> Unsubscribe {
            Unsubscribe {
                fixed_header: ::core::clone::Clone::clone(&self.fixed_header),
                packet_id: ::core::clone::Clone::clone(&self.packet_id),
                props: ::core::clone::Clone::clone(&self.props),
                topics: ::core::clone::Clone::clone(&self.topics),
            }
        }
    }
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for Unsubscribe {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for Unsubscribe {
        #[inline]
        fn eq(&self, other: &Unsubscribe) -> bool {
            self.packet_id == other.packet_id && self.fixed_header == other.fixed_header
                && self.props == other.props && self.topics == other.topics
        }
    }
    #[automatically_derived]
    impl ::core::cmp::Eq for Unsubscribe {
        #[inline]
        #[doc(hidden)]
        #[coverage(off)]
        fn assert_receiver_is_total_eq(&self) -> () {
            let _: ::core::cmp::AssertParamIsEq<codec::FixedHeader>;
            let _: ::core::cmp::AssertParamIsEq<u16>;
            let _: ::core::cmp::AssertParamIsEq<UserProperty>;
            let _: ::core::cmp::AssertParamIsEq<Vec<String>>;
        }
    }
    impl Unsubscribe {
        pub fn new_with_fixed_header(
            fixed_header: codec::FixedHeader,
        ) -> Result<Self, codec::MqttCodecError> {
            if fixed_header.packet_type != codec::PacketType::Unsubscribe {
                return Err(
                    MqttCodecError::new(
                        ::alloc::__export::must_use({
                                ::alloc::fmt::format(
                                    format_args!(
                                        "Unsuppprted PacketType for {0}",
                                        "#struct_name",
                                    ),
                                )
                            })
                            .as_str(),
                    ),
                );
            }
            Ok(Self {
                fixed_header,
                ..Default::default()
            })
        }
    }
    impl codec::CodecSize for Unsubscribe {
        fn codec_size(&self) -> u32 {
            use codec::PropertyCodecSize;
            let mut total_size = 0;
            total_size += 2;
            if !self.topics.is_empty() {
                total_size
                    += self.topics.iter().map(|item| item.codec_size()).sum::<u32>();
            }
            let property_size = self.property_size();
            total_size + property_size + codec::variable_byte_int_size(property_size)
        }
    }
    impl codec::PropertyCodecSize for Unsubscribe {
        fn property_size(&self) -> u32 {
            use codec::CodecSize;
            let mut property_size = 0;
            property_size += self.props.codec_size();
            property_size
        }
    }
    impl codec::Encode for Unsubscribe {
        fn encode(&self, dest: &mut bytes::BytesMut) -> Result<(), MqttCodecError> {
            use bytes::{BufMut, BytesMut};
            use codec::{CodecSize, PropertyCodecSize};
            self.fixed_header.encode(dest)?;
            codec::encode_variable_byte_int(self.codec_size(), dest)?;
            dest.put_u16(self.packet_id);
            codec::encode_variable_byte_int(self.property_size(), dest)?;
            self.props.encode(dest)?;
            for item in self.topics.iter() {
                item.encode(dest)?;
            }
            Ok(())
        }
    }
    impl codec::Decode for Unsubscribe {
        fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<u32, MqttCodecError> {
            use bytes::{BufMut, Buf, BytesMut};
            let mut bytes_read = 0;
            self.packet_id = src.get_u16();
            bytes_read += 2;
            let (property_length, var_bytes_read) = codec::decode_variable_byte_int(
                src,
            )?;
            bytes_read += var_bytes_read;
            let mut property_bytes_read = 0;
            while property_bytes_read < property_length {
                let property_type = src.get_u8().try_into()?;
                property_bytes_read += 1;
                match property_type {
                    PropertyType::UserProperty => {
                        property_bytes_read += self.props.decode(src)?;
                    }
                    _ => {
                        return Err(
                            codec::MqttCodecError::new_with_kind(
                                ::alloc::__export::must_use({
                                        ::alloc::fmt::format(
                                            format_args!(
                                                "MQTT v5 property type {0:?} is not supported",
                                                property_type,
                                            ),
                                        )
                                    })
                                    .as_str(),
                                codec::ErrorKind::UnsupportedProperty(property_type as u8),
                            ),
                        );
                    }
                }
            }
            bytes_read += property_bytes_read;
            Ok(bytes_read)
        }
    }
    impl Unsubscribe {
        pub fn new(packet_id: u16, topics: Vec<String>) -> Self {
            Self {
                packet_id,
                topics: topics.to_vec(),
                ..Default::default()
            }
        }
        pub fn add_topic(&mut self, topic: String) {
            self.topics.push(topic);
        }
    }
}
pub mod test {}
pub mod will {
    use crate::connect::Connect;
    use crate::property::UserProperty;
    use crate::publish::PayloadFormat;
    use crate::{
        codec::{self, PropertyCodecSize},
        MqttCodecError, PropertyType, QoSLevel,
    };
    use vaux_macro::{CodecSize, Decode, Encode, PropertyCodecSize};
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
    #[automatically_derived]
    impl ::core::fmt::Debug for WillHeader {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            let names: &'static _ = &[
                "will_delay",
                "payload_format",
                "message_expiry",
                "content_type",
                "response_topic",
                "correlation_data",
                "user_properties",
            ];
            let values: &[&dyn ::core::fmt::Debug] = &[
                &self.will_delay,
                &self.payload_format,
                &self.message_expiry,
                &self.content_type,
                &self.response_topic,
                &self.correlation_data,
                &&self.user_properties,
            ];
            ::core::fmt::Formatter::debug_struct_fields_finish(
                f,
                "WillHeader",
                names,
                values,
            )
        }
    }
    #[automatically_derived]
    impl ::core::default::Default for WillHeader {
        #[inline]
        fn default() -> WillHeader {
            WillHeader {
                will_delay: ::core::default::Default::default(),
                payload_format: ::core::default::Default::default(),
                message_expiry: ::core::default::Default::default(),
                content_type: ::core::default::Default::default(),
                response_topic: ::core::default::Default::default(),
                correlation_data: ::core::default::Default::default(),
                user_properties: ::core::default::Default::default(),
            }
        }
    }
    #[automatically_derived]
    impl ::core::clone::Clone for WillHeader {
        #[inline]
        fn clone(&self) -> WillHeader {
            WillHeader {
                will_delay: ::core::clone::Clone::clone(&self.will_delay),
                payload_format: ::core::clone::Clone::clone(&self.payload_format),
                message_expiry: ::core::clone::Clone::clone(&self.message_expiry),
                content_type: ::core::clone::Clone::clone(&self.content_type),
                response_topic: ::core::clone::Clone::clone(&self.response_topic),
                correlation_data: ::core::clone::Clone::clone(&self.correlation_data),
                user_properties: ::core::clone::Clone::clone(&self.user_properties),
            }
        }
    }
    #[automatically_derived]
    impl ::core::cmp::Eq for WillHeader {
        #[inline]
        #[doc(hidden)]
        #[coverage(off)]
        fn assert_receiver_is_total_eq(&self) -> () {
            let _: ::core::cmp::AssertParamIsEq<Option<u32>>;
            let _: ::core::cmp::AssertParamIsEq<Option<PayloadFormat>>;
            let _: ::core::cmp::AssertParamIsEq<Option<u32>>;
            let _: ::core::cmp::AssertParamIsEq<Option<String>>;
            let _: ::core::cmp::AssertParamIsEq<Option<String>>;
            let _: ::core::cmp::AssertParamIsEq<Vec<u8>>;
            let _: ::core::cmp::AssertParamIsEq<UserProperty>;
        }
    }
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for WillHeader {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for WillHeader {
        #[inline]
        fn eq(&self, other: &WillHeader) -> bool {
            self.will_delay == other.will_delay
                && self.payload_format == other.payload_format
                && self.message_expiry == other.message_expiry
                && self.content_type == other.content_type
                && self.response_topic == other.response_topic
                && self.correlation_data == other.correlation_data
                && self.user_properties == other.user_properties
        }
    }
    impl codec::Encode for WillHeader {
        fn encode(&self, dest: &mut bytes::BytesMut) -> Result<(), MqttCodecError> {
            use bytes::{BufMut, BytesMut};
            codec::encode_variable_byte_int(self.property_size(), dest)?;
            if let Some(v) = self.will_delay {
                dest.put_u8(PropertyType::WillDelay as u8);
                dest.put_u32(v);
            }
            if let Some(payload_format) = self.payload_format.as_ref() {
                dest.put_u8(PropertyType::PayloadFormat as u8);
                payload_format.encode(dest)?;
            }
            if let Some(v) = self.message_expiry {
                dest.put_u8(PropertyType::MessageExpiry as u8);
                dest.put_u32(v);
            }
            if let Some(v) = self.content_type.as_ref() {
                dest.put_u8(PropertyType::ContentType as u8);
                codec::encode_string(v, dest)?;
            }
            if let Some(v) = self.response_topic.as_ref() {
                dest.put_u8(PropertyType::ResponseTopic as u8);
                codec::encode_string(v, dest)?;
            }
            if !Vec::is_empty(&self.correlation_data) {
                dest.put_u8(PropertyType::CorrelationData as u8);
                codec::encode_array_field(&self.correlation_data, dest)?;
            }
            self.user_properties.encode(dest)?;
            Ok(())
        }
    }
    impl codec::Decode for WillHeader {
        fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<u32, MqttCodecError> {
            use bytes::{BufMut, Buf, BytesMut};
            let mut bytes_read = 0;
            let (property_length, var_bytes_read) = codec::decode_variable_byte_int(
                src,
            )?;
            bytes_read += var_bytes_read;
            let mut property_bytes_read = 0;
            while property_bytes_read < property_length {
                let property_type = src.get_u8().try_into()?;
                property_bytes_read += 1;
                match property_type {
                    PropertyType::WillDelay => {
                        let mut value = u32::default();
                        property_bytes_read += value.decode(src)?;
                        self.will_delay = Some(value);
                    }
                    PropertyType::PayloadFormat => {
                        let mut value = PayloadFormat::default();
                        property_bytes_read += value.decode(src)?;
                        self.payload_format = Some(value);
                    }
                    PropertyType::MessageExpiry => {
                        let mut value = u32::default();
                        property_bytes_read += value.decode(src)?;
                        self.message_expiry = Some(value);
                    }
                    PropertyType::ContentType => {
                        let mut value = String::default();
                        property_bytes_read += value.decode(src)?;
                        self.content_type = Some(value);
                    }
                    PropertyType::ResponseTopic => {
                        let mut value = String::default();
                        property_bytes_read += value.decode(src)?;
                        self.response_topic = Some(value);
                    }
                    PropertyType::CorrelationData => {
                        let (value, var_bytes_read) = codec::decode_array_field(src)?;
                        bytes_read += var_bytes_read;
                        self.correlation_data = value;
                    }
                    PropertyType::UserProperty => {
                        property_bytes_read += self.user_properties.decode(src)?;
                    }
                    _ => {
                        return Err(
                            codec::MqttCodecError::new_with_kind(
                                ::alloc::__export::must_use({
                                        ::alloc::fmt::format(
                                            format_args!(
                                                "MQTT v5 property type {0:?} is not supported",
                                                property_type,
                                            ),
                                        )
                                    })
                                    .as_str(),
                                codec::ErrorKind::UnsupportedProperty(property_type as u8),
                            ),
                        );
                    }
                }
            }
            bytes_read += property_bytes_read;
            Ok(bytes_read)
        }
    }
    impl codec::PropertyCodecSize for WillHeader {
        fn property_size(&self) -> u32 {
            use codec::CodecSize;
            let mut property_size = 0;
            if let Some(_) = &self.will_delay {
                property_size += 1 + 4;
            }
            if let Some(field) = &self.payload_format {
                property_size += field.codec_size();
            }
            if let Some(_) = &self.message_expiry {
                property_size += 1 + 4;
            }
            if let Some(field_name) = &self.content_type {
                let value_size = field_name.len() as u32 + 2;
                property_size += 1 + value_size;
            }
            if let Some(field_name) = &self.response_topic {
                let value_size = field_name.len() as u32 + 2;
                property_size += 1 + value_size;
            }
            if !Vec::is_empty(&self.correlation_data) {
                if !self.correlation_data.is_empty() {
                    property_size += 1 + 2 + self.correlation_data.len() as u32;
                }
            }
            property_size += self.user_properties.codec_size();
            property_size
        }
    }
    impl codec::CodecSize for WillHeader {
        fn codec_size(&self) -> u32 {
            use codec::PropertyCodecSize;
            let mut total_size = 0;
            let property_size = self.property_size();
            total_size + property_size + codec::variable_byte_int_size(property_size)
        }
    }
    pub struct WillMessage {
        pub header: WillHeader,
        pub topic: String,
        pub payload: Vec<u8>,
        pub qos: QoSLevel,
        pub retain: bool,
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for WillMessage {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::debug_struct_field5_finish(
                f,
                "WillMessage",
                "header",
                &self.header,
                "topic",
                &self.topic,
                "payload",
                &self.payload,
                "qos",
                &self.qos,
                "retain",
                &&self.retain,
            )
        }
    }
    #[automatically_derived]
    impl ::core::default::Default for WillMessage {
        #[inline]
        fn default() -> WillMessage {
            WillMessage {
                header: ::core::default::Default::default(),
                topic: ::core::default::Default::default(),
                payload: ::core::default::Default::default(),
                qos: ::core::default::Default::default(),
                retain: ::core::default::Default::default(),
            }
        }
    }
    #[automatically_derived]
    impl ::core::clone::Clone for WillMessage {
        #[inline]
        fn clone(&self) -> WillMessage {
            WillMessage {
                header: ::core::clone::Clone::clone(&self.header),
                topic: ::core::clone::Clone::clone(&self.topic),
                payload: ::core::clone::Clone::clone(&self.payload),
                qos: ::core::clone::Clone::clone(&self.qos),
                retain: ::core::clone::Clone::clone(&self.retain),
            }
        }
    }
    #[automatically_derived]
    impl ::core::cmp::Eq for WillMessage {
        #[inline]
        #[doc(hidden)]
        #[coverage(off)]
        fn assert_receiver_is_total_eq(&self) -> () {
            let _: ::core::cmp::AssertParamIsEq<WillHeader>;
            let _: ::core::cmp::AssertParamIsEq<String>;
            let _: ::core::cmp::AssertParamIsEq<Vec<u8>>;
            let _: ::core::cmp::AssertParamIsEq<QoSLevel>;
            let _: ::core::cmp::AssertParamIsEq<bool>;
        }
    }
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for WillMessage {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for WillMessage {
        #[inline]
        fn eq(&self, other: &WillMessage) -> bool {
            self.retain == other.retain && self.header == other.header
                && self.topic == other.topic && self.payload == other.payload
                && self.qos == other.qos
        }
    }
    impl From<WillMessage> for Connect {
        fn from(will: WillMessage) -> Self {
            let mut connect = Connect::default();
            connect.set_will_message(will);
            connect
        }
    }
    impl WillMessage {
        pub fn new(topic: String, payload: &[u8], qos: QoSLevel, retain: bool) -> Self {
            WillMessage {
                header: WillHeader::default(),
                topic,
                payload: payload.to_vec(),
                qos,
                retain,
            }
        }
        pub fn with_delay(mut self, delay: u32) -> Self {
            self.header.will_delay = Some(delay);
            self
        }
        pub fn with_payload_format(mut self, format: PayloadFormat) -> Self {
            self.header.payload_format = Some(format);
            self
        }
        pub fn with_message_expiry(mut self, expiry: u32) -> Self {
            self.header.message_expiry = Some(expiry);
            self
        }
        pub fn with_content_type(mut self, content_type: String) -> Self {
            self.header.content_type = Some(content_type);
            self
        }
        pub fn with_response_topic(mut self, response_topic: String) -> Self {
            self.header.response_topic = Some(response_topic);
            self
        }
        pub fn with_correlation_data(mut self, correlation_data: Vec<u8>) -> Self {
            self.header.correlation_data = correlation_data;
            self
        }
        pub fn with_user_properties(mut self, user_properties: UserProperty) -> Self {
            self.header.user_properties = user_properties;
            self
        }
    }
}
pub use codec::{MqttCodecError, Packet, PacketType, QoSLevel, Reason};
use vaux_macro::packet;
pub use {
    connack::ConnAck, connect::Connect, disconnect::Disconnect, property::PropertyType,
    publish::{PayloadFormat, Publish},
    pubresp::{PubAck, PubComp, PubRec, PubRel},
    subscribe::{Subscribe, SubscriptionFilter},
    will::{WillHeader, WillMessage},
    codec::fixed::FixedHeader,
};
pub enum MqttVersion {
    V3,
    V5,
}
impl std::fmt::Display for MqttVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            MqttVersion::V3 => f.write_fmt(format_args!("v3.1.1")),
            MqttVersion::V5 => f.write_fmt(format_args!("v5.0")),
        }
    }
}
pub struct MqttError {
    pub version: Option<MqttVersion>,
    pub section: Option<String>,
    pub message: String,
}
impl MqttError {
    pub fn new(message: &str) -> MqttError {
        MqttError {
            version: None,
            section: None,
            message: message.to_string(),
        }
    }
    pub fn new_from_spec(
        version: MqttVersion,
        section: &str,
        message: &str,
    ) -> MqttError {
        MqttError {
            version: Some(version),
            section: Some(section.to_string()),
            message: message.to_string(),
        }
    }
}
impl std::fmt::Display for MqttError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if let Some(version) = &self.version {
            f.write_fmt(format_args!("MQTT{0} ", version))?;
        }
        if let Some(section) = &self.section {
            f.write_fmt(format_args!(" {0}: ", section))?;
        }
        f.write_fmt(format_args!("{0}", self.message))
    }
}
pub struct PingReq {
    pub fixed_header: codec::FixedHeader,
}
#[automatically_derived]
impl ::core::default::Default for PingReq {
    #[inline]
    fn default() -> PingReq {
        PingReq {
            fixed_header: ::core::default::Default::default(),
        }
    }
}
#[automatically_derived]
impl ::core::fmt::Debug for PingReq {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field1_finish(
            f,
            "PingReq",
            "fixed_header",
            &&self.fixed_header,
        )
    }
}
#[automatically_derived]
impl ::core::clone::Clone for PingReq {
    #[inline]
    fn clone(&self) -> PingReq {
        PingReq {
            fixed_header: ::core::clone::Clone::clone(&self.fixed_header),
        }
    }
}
#[automatically_derived]
impl ::core::cmp::Eq for PingReq {
    #[inline]
    #[doc(hidden)]
    #[coverage(off)]
    fn assert_receiver_is_total_eq(&self) -> () {
        let _: ::core::cmp::AssertParamIsEq<codec::FixedHeader>;
    }
}
#[automatically_derived]
impl ::core::marker::StructuralPartialEq for PingReq {}
#[automatically_derived]
impl ::core::cmp::PartialEq for PingReq {
    #[inline]
    fn eq(&self, other: &PingReq) -> bool {
        self.fixed_header == other.fixed_header
    }
}
impl PingReq {
    pub fn new_with_fixed_header(
        fixed_header: codec::FixedHeader,
    ) -> Result<Self, codec::MqttCodecError> {
        if fixed_header.packet_type != codec::PacketType::PingReq {
            return Err(
                MqttCodecError::new(
                    ::alloc::__export::must_use({
                            ::alloc::fmt::format(
                                format_args!(
                                    "Unsuppprted PacketType for {0}",
                                    "#struct_name",
                                ),
                            )
                        })
                        .as_str(),
                ),
            );
        }
        Ok(Self {
            fixed_header,
            ..Default::default()
        })
    }
}
impl codec::CodecSize for PingReq {
    fn codec_size(&self) -> u32 {
        let mut total_size = 0;
        total_size
    }
}
impl codec::Encode for PingReq {
    fn encode(&self, dest: &mut bytes::BytesMut) -> Result<(), MqttCodecError> {
        use bytes::{BufMut, BytesMut};
        use codec::{CodecSize, PropertyCodecSize};
        self.fixed_header.encode(dest)?;
        codec::encode_variable_byte_int(self.codec_size(), dest)?;
        Ok(())
    }
}
impl codec::Decode for PingReq {
    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<u32, MqttCodecError> {
        use bytes::{BufMut, Buf, BytesMut};
        let mut bytes_read = 0;
        Ok(bytes_read)
    }
}
pub struct PingResp {
    pub fixed_header: codec::FixedHeader,
}
#[automatically_derived]
impl ::core::default::Default for PingResp {
    #[inline]
    fn default() -> PingResp {
        PingResp {
            fixed_header: ::core::default::Default::default(),
        }
    }
}
#[automatically_derived]
impl ::core::fmt::Debug for PingResp {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field1_finish(
            f,
            "PingResp",
            "fixed_header",
            &&self.fixed_header,
        )
    }
}
#[automatically_derived]
impl ::core::clone::Clone for PingResp {
    #[inline]
    fn clone(&self) -> PingResp {
        PingResp {
            fixed_header: ::core::clone::Clone::clone(&self.fixed_header),
        }
    }
}
#[automatically_derived]
impl ::core::cmp::Eq for PingResp {
    #[inline]
    #[doc(hidden)]
    #[coverage(off)]
    fn assert_receiver_is_total_eq(&self) -> () {
        let _: ::core::cmp::AssertParamIsEq<codec::FixedHeader>;
    }
}
#[automatically_derived]
impl ::core::marker::StructuralPartialEq for PingResp {}
#[automatically_derived]
impl ::core::cmp::PartialEq for PingResp {
    #[inline]
    fn eq(&self, other: &PingResp) -> bool {
        self.fixed_header == other.fixed_header
    }
}
impl PingResp {
    pub fn new_with_fixed_header(
        fixed_header: codec::FixedHeader,
    ) -> Result<Self, codec::MqttCodecError> {
        if fixed_header.packet_type != codec::PacketType::PingResp {
            return Err(
                MqttCodecError::new(
                    ::alloc::__export::must_use({
                            ::alloc::fmt::format(
                                format_args!(
                                    "Unsuppprted PacketType for {0}",
                                    "#struct_name",
                                ),
                            )
                        })
                        .as_str(),
                ),
            );
        }
        Ok(Self {
            fixed_header,
            ..Default::default()
        })
    }
}
impl codec::CodecSize for PingResp {
    fn codec_size(&self) -> u32 {
        let mut total_size = 0;
        total_size
    }
}
impl codec::Encode for PingResp {
    fn encode(&self, dest: &mut bytes::BytesMut) -> Result<(), MqttCodecError> {
        use bytes::{BufMut, BytesMut};
        use codec::{CodecSize, PropertyCodecSize};
        self.fixed_header.encode(dest)?;
        codec::encode_variable_byte_int(self.codec_size(), dest)?;
        Ok(())
    }
}
impl codec::Decode for PingResp {
    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<u32, MqttCodecError> {
        use bytes::{BufMut, Buf, BytesMut};
        let mut bytes_read = 0;
        Ok(bytes_read)
    }
}
