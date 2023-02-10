use bytes::BytesMut;

use crate::{buffer::Buffer, property::Property, PropertyType};

#[test]
fn get_u16() {
    let mut data: Vec<u8> = vec![0b_0011_0000, 0b_0011_1001];
    let mut src = Buffer::new(&mut data);

    let value = src.get_u16();
    assert_eq!(12_345_u16, value);
}

#[test]
fn get_u32() {
    let mut data: Vec<u8> = vec![0b_0000_0111, 0b_0101_1011, 0b_1100_1101, 0b_0001_0101];
    let mut src = Buffer::new(&mut data);

    let value = src.get_u32();
    assert_eq!(123_456_789_u32, value);
}

#[test]
fn get_var_uint() {
    let mut data: Vec<u8> = vec![0x80, 0x80, 0x80, 0x01];
    let mut src = Buffer::new(&mut data);

    let value = src.get_var_u32();
    assert_eq!(2_097_152_u32, value);
}

#[test]
fn get_utf8() {
    let mut data: Vec<u8> = vec![0x00, 0x05, 0x68, 0x65, 0x6c, 0x6c, 0x6f];
    let mut src = Buffer::new(&mut data);

    match src.get_utf8_as_ref() {
        Ok(message) => assert_eq!("hello", message),
        Err(e) => panic!("{}", e.to_string()),
    }
}

#[test]
fn get_bin() {
    println!("Property Size is {}", std::mem::size_of::<Property>());
    println!("PropertyType Size is {}", std::mem::size_of::<PropertyType>());
    let mut data: Vec<u8> = vec![
        0x00, 0x0A, 0x0A, 0x09, 0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01,
    ];
    let check = data.clone();
    let mut src = Buffer::new(&mut data);
    match src.get_bin_as_ref() {
        Ok(payload) => {
            for (i, v) in payload.iter().enumerate() {
                assert_eq!(check[i + 2], *v)
            }
        }
        Err(e) => panic!("{}", e.to_string()),
    }
}
