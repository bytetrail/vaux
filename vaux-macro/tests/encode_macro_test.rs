use vaux_macro::{CodecSize, Encode};

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
