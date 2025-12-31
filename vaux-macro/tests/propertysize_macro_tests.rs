use vaux_macro::PropertyCodecSize;

enum TestProperty {
    PropertyOne,
}

trait CodecSize {
    fn codec_size(&self) -> u32;
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
fn test_property_size_impl_for_disconnect() {
    #[derive(PropertyCodecSize)]
    struct Disconnect {
        #[property(property_type = "TestProperty::PropertyOne")]
        reason: Option<String>,
        #[property(property_type = "TestProperty::PropertyTwo")]
        property_one: Option<u16>,
    }

    let disconnect = Disconnect {
        reason: Some("Some reason".to_string()),
        property_one: Some(42),
    };

    assert_eq!(disconnect.property_size(), 17); // 1 byte for property identifier +  2 bytes for length + 11 bytes for "Some reason" + 1 byte for property identifier + 2 bytes for u16 value
}

#[test]
fn test_vec_property_size_impl() {
    #[derive(PropertyCodecSize)]
    struct TestStruct {
        #[property(property_type = "TestProperty::PropertyOne")]
        data: Vec<u8>,
    }

    let test_instance = TestStruct {
        data: vec![1, 2, 3, 4, 5],
    };

    assert_eq!(test_instance.property_size(), 8); // 1 byte for property identifier + 2 bytes for length + 5 bytes for data

    #[derive(PropertyCodecSize)]
    struct TestStructTwo {
        #[property(property_type = "TestProperty::PropertyOne")]
        data: Option<Vec<u8>>,
    }

    let test_instance_none = TestStructTwo { data: None };
    assert_eq!(test_instance_none.property_size(), 0); // 1 byte for property identifier only
    let test_instance_some = TestStructTwo {
        data: Some(vec![1, 2, 3]),
    };
    assert_eq!(test_instance_some.property_size(), 6); // 1 byte for property identifier + 2 bytes for length + 3 bytes for data
}
