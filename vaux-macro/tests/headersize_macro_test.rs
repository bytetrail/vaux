use vaux_macro::{HeaderCodecSize, PropertyCodecSize};

trait HeaderCodecSize {
    fn header_size(&self) -> u32;
}

trait PropertyCodecSize {
    fn property_size(&self) -> u32;
}

fn variable_byte_int_size(value: u32) -> u32 {
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

#[test]
fn test_size_impl_for_string() {
    #[derive(PropertyCodecSize, HeaderCodecSize)]
    struct TestStruct {
        test_string: String,
    }
    let test_instance = TestStruct {
        test_string: "hello".to_string(),
    };
    assert_eq!(test_instance.header_size(), 8); // 2 bytes for length + 5 bytes for "hello" + 1 byte for property size wrapper
}

#[test]
fn test_size_impl_for_option_string() {
    #[derive(HeaderCodecSize, PropertyCodecSize)]
    struct TestStruct {
        optional_string: Option<String>,
    }
    let test_instance_some = TestStruct {
        optional_string: Some("world".to_string()),
    };
    let test_instance_none = TestStruct {
        optional_string: None,
    };
    assert_eq!(test_instance_some.header_size(), 8); // 2 bytes for length + 5 bytes for "world"
    assert_eq!(test_instance_none.header_size(), 1); // None should result in size 1 for property size wrapper
}

#[test]
pub fn test_size_impl_for_primitive_types() {
    #[derive(HeaderCodecSize, PropertyCodecSize)]
    struct TestStruct {
        _a: u8,
        _b: u16,
        _c: u32,
        _d: i8,
        _e: i16,
        _f: i32,
    }

    let test_instance = TestStruct {
        _a: 1,
        _b: 2,
        _c: 3,
        _d: -1,
        _e: -2,
        _f: -3,
    };

    let expected_size = 1 + 2 + 4 + 1 + 2 + 4 + 1; // sum of sizes of all fields + property size wrapper
    assert_eq!(test_instance.header_size(), expected_size);
}

#[test]
fn test_size_impl_for_option_primitive_types() {
    #[derive(HeaderCodecSize, PropertyCodecSize)]
    struct TestStruct {
        _a: Option<u8>,
        _b: Option<u16>,
        _c: Option<u32>,
    }

    let test_instance_some = TestStruct {
        _a: Some(1),
        _b: Some(2),
        _c: Some(3),
    };

    let test_instance_none = TestStruct {
        _a: None,
        _b: None,
        _c: None,
    };

    let expected_size_some = 1 + 2 + 4 + 1; // sum of sizes of all fields + property size wrapper
    assert_eq!(test_instance_some.header_size(), expected_size_some);
    assert_eq!(test_instance_none.header_size(), 1); // None should result in size 1 for property size wrapper
}

#[test]
fn test_size_impl_struct() {
    #[derive(HeaderCodecSize, PropertyCodecSize)]
    struct TestStruct {
        test_string: String,
        optional_string: Option<String>,
        optional_w_none: Option<String>,
        _a: u8,
        _b: Option<u16>,
    }

    let test_instance = TestStruct {
        test_string: "hello".to_string(),
        optional_string: Some("world".to_string()),
        optional_w_none: None,
        _a: 1,
        _b: Some(2),
    };

    let expected_size = 7 + 7 + 0 + 1 + 2 + 1; // size of test_string + optional_string + optianl_w_none + _a + _b + property size wrapper
    assert_eq!(test_instance.header_size(), expected_size);
}
