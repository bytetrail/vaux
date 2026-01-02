use bytes::BufMut;
use vaux_macro::{CodecSize, Encode};

mod codec {
    pub use crate::MqttCodecError;
    use bytes::BufMut;
    pub use bytes::BytesMut;

    pub fn put_utf8(val: &str, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
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

    pub fn put_var_u32(mut val: u32, dest: &mut BytesMut) {
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

#[derive(Default, Debug)]
pub struct MqttCodecError {
    pub reason: String,
    pub kind: ErrorKind,
}

trait CodecSize {
    fn codec_size(&self) -> u32;
}

trait Encode {
    fn encode(&mut self, dest: &mut bytes::BytesMut) -> Result<(), MqttCodecError>;
}

#[test]
fn test_primitive_encode_impl() {
    #[derive(CodecSize, Encode)]
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
    #[derive(CodecSize, Encode)]
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
    #[derive(CodecSize, Encode)]
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

    #[derive(CodecSize, Encode)]
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
