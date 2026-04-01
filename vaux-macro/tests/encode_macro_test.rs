use bytes::BufMut;
use vaux_macro::{CodecSize, Encode, PropertyCodecSize, PropertyEncode};

mod codec {
    pub use crate::MqttCodecError;
    use bytes::BufMut;
    pub use bytes::BytesMut;

    pub trait PropertyCodecSize {
        fn property_size(&self) -> u32;
    }

    pub trait CodecSize : PropertyCodecSize {
        fn codec_size(&self) -> u32;
    }

    
    pub trait PropertyEncode {
        fn property_encode(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError>;
    }

    pub trait PropertyDecode {
        fn property_decode(&mut self, src: &mut BytesMut) -> Result<usize, MqttCodecError>;
    }

    pub trait Encode: PropertyEncode {
        fn encode(&self, dest: &mut bytes::BytesMut) -> Result<(), MqttCodecError>;
    }

    pub fn variable_byte_int_size(value: u32) -> u32 {
        if value < 128 {
            1
        } else if value < 16384 {
            2
        } else if value < 2097152 {
            3
        } else {
            4
        }
    }

    pub fn encode_string(val: &str, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
        let len = val.len();
        if len > u16::MAX as usize {
            return Err(MqttCodecError {
                reason: "String length exceeds u16 max".to_string(),
                kind: crate::ErrorKind::InvalidUTF8,
            });
        }
        put_u16(len as u16, dest);
        dest.put_slice(val.as_bytes());
        Ok(())
    }

    pub fn encode_variable_byte_int(
        mut val: u32,
        dest: &mut BytesMut,
    ) -> Result<(), MqttCodecError> {
        loop {
            let mut byte = (val % 128) as u8;
            val /= 128;
            if val > 0 {
                byte |= 0x80;
            }
            dest.put_u8(byte);
            if val == 0 {
                break;
            }
        }
        Ok(())
    }

    pub fn put_u8(val: u8, dest: &mut BytesMut) {
        dest.put_u8(val);
    }
    pub fn put_u16(val: u16, dest: &mut BytesMut) {
        dest.put_u16(val);
    }
    pub fn put_u32(val: u32, dest: &mut BytesMut) {
        dest.put_u32(val);
    }
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub enum ErrorKind {
    InsufficientData(usize, usize),
    #[default]
    MalformedPacket,
    UnsupportedQosLevel,
    UnsupportedResponseType,
    UnsupportedReason,
    //UnsupportedProperty(PropertyType),
    InvalidUTF8,
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub enum TestProperty {
    #[default]
    PropertyOne,
    PropertyTwo,
    PropertyThree,
    PropertyFour,
    PropertyFive,
}

#[derive(Default, Debug)]
pub struct MqttCodecError {
    pub reason: String,
    pub kind: ErrorKind,
}

#[test]
fn test_primitive_encode_impl() {
    use crate::codec::{PropertyEncode, CodecSize, PropertyCodecSize, Encode};

    #[derive(CodecSize, PropertyCodecSize, PropertyEncode, Encode)]
    struct TestStruct {
        _a: u8,
        _b: u16,
        _c: u32,
    }

    let mut test_struct = TestStruct {
        _a: 1,
        _b: 15000,
        _c: 150000,
    };

    let dest = &mut bytes::BytesMut::new();
    test_struct.encode(dest).unwrap();
    // verify that encoding was successful
    assert_eq!(dest.len(), 7); // 1 byte for u8 + 2 bytes for u16 + 4 bytes for u32

    // u8
    assert_eq!(dest[0], 1);
    // 15000 as u16
    assert_eq!(&dest[1..3], &[0x3A, 0x98]);
    // 150000 as u32
    assert_eq!(&dest[3..7], &[0x00, 0x02, 0x49, 0xF0]);
}

#[test]
fn test_string_encode_impl() {
    use crate::codec::{PropertyEncode, CodecSize, PropertyCodecSize, Encode};

    #[derive(CodecSize, PropertyCodecSize, PropertyEncode, Encode)]
    struct TestStruct {
        test_string: String,
    }
    let mut test_instance = TestStruct {
        test_string: "hello".to_string(),
    };
    let dest = &mut bytes::BytesMut::new();
    test_instance.encode(dest).unwrap();
    // verify that encoding was successful
    assert_eq!(dest.len(), 7); // 2 bytes for length + 5
                               // length of "hello" as u16
    assert_eq!(&dest[0..2], &[0x00, 0x05]);
    // "hello" bytes
    assert_eq!(&dest[2..7], b"hello");
}

#[test]
fn test_option_string_encode_impl() {
    use crate::codec::{PropertyEncode, CodecSize, PropertyCodecSize, Encode};

    #[derive(PropertyCodecSize, CodecSize, PropertyEncode, Encode)]
    struct TestStruct {
        optional_string: Option<String>,
    }
    let mut test_instance_some = TestStruct {
        optional_string: Some("world".to_string()),
    };
    let dest_some = &mut bytes::BytesMut::new();
    test_instance_some.encode(dest_some).unwrap();
    // verify that encoding was successful
    assert_eq!(dest_some.len(), 7); // 2 bytes for length + 5
                                    // length of "world" as u16
    assert_eq!(&dest_some[0..2], &[0x00, 0x05]);
    // "world" bytes
    assert_eq!(&dest_some[2..7], b"world");
    let mut test_instance_none = TestStruct {
        optional_string: None,
    };
    let dest_none = &mut bytes::BytesMut::new();
    test_instance_none.encode(dest_none).unwrap();
    // verify that encoding was successful
    assert_eq!(dest_none.len(), 0); // None should result in no bytes encoded
}

#[test]
fn test_custom_encode_with() {
    use crate::codec::{PropertyEncode, CodecSize, PropertyCodecSize, Encode};

    fn custom_size(value: &u64) -> u32 {
        4 // Custom size logic: for example, always 4 bytes
    }

    fn custom_encode(value: &u64, dest: &mut bytes::BytesMut) -> Result<(), MqttCodecError> {
        // Custom encoding logic: for example, encode as u16 followed by u16
        let high = (value >> 16) as u16;
        let low = (value & 0xFFFF) as u16;
        dest.put_u16(high);
        dest.put_u16(low);
        Ok(())
    }

    #[derive(CodecSize, PropertyCodecSize, PropertyEncode,Encode)]
    struct TestStruct {
        #[codec(size_with = "custom_size", encode_with = "custom_encode")]
        custom_encoded_field: u64,
    }

    let mut test_instance = TestStruct {
        custom_encoded_field: 0x12345678,
    };
    let dest = &mut bytes::BytesMut::new();
    test_instance.encode(dest).unwrap();
    // verify that encoding was successful
    assert_eq!(dest.len(), 4); // 2 bytes for high u16 + 2 bytes for low u16

    // high part (0x1234)
    assert_eq!(&dest[0..2], &[0x12, 0x34]);
    // low part (0x5678)
    assert_eq!(&dest[2..4], &[0x56, 0x78]);
}

fn is_zero(s: &u32) -> bool {
    *s == 0
}

#[test]
fn test_skip_if_impl() {
    use crate::codec::{PropertyEncode, CodecSize, PropertyCodecSize, Encode};

    #[derive(CodecSize, PropertyCodecSize, PropertyEncode, Encode)]
    struct TestStruct {
        #[codec(skip_if = "is_zero")]
        optional_w_none: u32,
        #[codec(skip_if = "String::is_empty")]
        optional_string: String,
    }

    let test_instance = TestStruct {
        optional_w_none: 0,
        optional_string: "".to_string(),
    };

    let test_instance_some = TestStruct {
        optional_w_none: 5,
        optional_string: "hello".to_string(),
    };

    let expected_size_some = 4 + 2 + 5; // size of optional_w_none + length of optional_string + string bytes
    assert_eq!(test_instance_some.codec_size(), expected_size_some);
    let expected_size_none = 0; // both fields are skipped
    assert_eq!(test_instance.codec_size(), expected_size_none);
}

#[test]
fn test_property_skip_if_impl() {
    use crate::codec::{PropertyEncode, CodecSize, PropertyCodecSize, Encode};

    fn is_zero(s: &u32) -> bool {
        *s == 0
    }

    #[derive(PropertyCodecSize, CodecSize, PropertyEncode, Encode)]
    struct TestStruct {
        #[codec(property_type = "TestProperty::PropertyOne")]
        #[codec(skip_if = "is_zero")]
        optional_w_none: u32,
        #[codec(property_type = "TestProperty::PropertyTwo")]
        #[codec(skip_if = "String::is_empty")]
        optional_string: String,
    }

    let test_instance = TestStruct {
        optional_w_none: 0,
        optional_string: "".to_string(),
    };

    let test_instance_some = TestStruct {
        optional_w_none: 5,
        optional_string: "hello".to_string(),
    };

    let expected_size_some = 1 + 4 + 1 + 2 + 5; // property id + size of optional_w_none + property id + length of optional_string + string bytes
    assert_eq!(test_instance_some.property_size(), expected_size_some);
    let expected_size_none = 0; // both fields are skipped
    assert_eq!(test_instance.property_size(), expected_size_none);
    // codec size should be 1 for all skipped properties
    let expected_codec_size = 1;
    assert_eq!(test_instance.codec_size(), expected_codec_size);
    // expected codec size for some
    // property length field + prop id + size of optional_w_none + prop id + length of optional_string + string bytes
    let expected_codec_size_some = 1 + 1 + 4 + 1 + 2 + 5;
    assert_eq!(test_instance_some.codec_size(), expected_codec_size_some);
}
