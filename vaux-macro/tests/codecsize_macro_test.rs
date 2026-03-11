use vaux_macro::{CodecSize, PropertyCodecSize};

pub mod codec {
    pub trait CodecSize {
        fn codec_size(&self) -> u32;
    }

    pub trait PropertyCodecSize {
        fn property_size(&self) -> u32;
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
}

#[test]
fn test_size_impl_for_string() {
    use crate::codec::CodecSize;

    #[derive(CodecSize)]
    struct TestStruct {
        test_string: String,
    }
    let test_instance = TestStruct {
        test_string: "hello".to_string(),
    };
    assert_eq!(test_instance.codec_size(), 7); // 2 bytes for length + 5 bytes for "hello"
}

#[test]
fn test_size_impl_for_option_string() {
    use crate::codec::CodecSize;
    use crate::codec::PropertyCodecSize;

    #[derive(CodecSize, PropertyCodecSize)]
    struct TestStruct {
        optional_string: Option<String>,
    }
    let test_instance_some = TestStruct {
        optional_string: Some("world".to_string()),
    };
    let test_instance_none = TestStruct {
        optional_string: None,
    };
    assert_eq!(test_instance_some.codec_size(), 7); // 2 bytes for length + 5 bytes for "world"
    assert_eq!(test_instance_none.codec_size(), 0); // None should result in size 0 for codec size
}

#[test]
pub fn test_size_impl_for_primitive_types() {
    use crate::codec::CodecSize;
    #[derive(CodecSize)]
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

    let expected_size = 1 + 2 + 4 + 1 + 2 + 4; // sum of sizes of all fields + property size wrapper
    assert_eq!(test_instance.codec_size(), expected_size);
}

#[test]
fn test_size_impl_for_option_primitive_types() {
    use crate::codec::CodecSize;
    #[derive(CodecSize)]
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

    let expected_size_some = 1 + 2 + 4; // sum of sizes of all fields
    assert_eq!(test_instance_some.codec_size(), expected_size_some);
    assert_eq!(test_instance_none.codec_size(), 0); // None should result in size 0 for codec size
}

#[test]
fn test_size_impl_struct() {
    use crate::codec::CodecSize;
    #[derive(CodecSize)]
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

    let expected_size = 7 + 7 + 0 + 1 + 2; // size of test_string + optional_string + optianl_w_none + _a + _b
    assert_eq!(test_instance.codec_size(), expected_size);
}

fn custom_size(value: &u32) -> u32 {
    *value + 10
}

#[test]
fn test_size_impl_size_with() {
    use crate::codec::CodecSize;
    #[derive(CodecSize)]
    struct TestStruct {
        #[codec(size_with = "custom_size")]
        custom_sized_field: u32,
    }

    let test_instance = TestStruct {
        custom_sized_field: 5,
    };

    let expected_size = custom_size(&test_instance.custom_sized_field); // should be 5 + 10 = 15
    assert_eq!(test_instance.codec_size(), expected_size);
}

#[test]
fn test_skip_if_impl() {
    use crate::codec::CodecSize;

    fn is_zero(s: &u32) -> bool {
        *s == 0
    }

    #[derive(CodecSize)]
    struct TestStruct {
        #[codec(skip_if = "is_zero")]
        field_one: u32,
        field_two: u16,
    }

    let test_instance_skip = TestStruct {
        field_one: 0,
        field_two: 42,
    };

    let test_instance_no_skip = TestStruct {
        field_one: 10,
        field_two: 42,
    };

    let expected_size_skip = 2; // only field_two size
    let expected_size_no_skip = 4 + 2; // field_one + field_two sizes

    assert_eq!(test_instance_skip.codec_size(), expected_size_skip);
    assert_eq!(test_instance_no_skip.codec_size(), expected_size_no_skip);
}


#[test]
fn test_complex_struct_size() {
    use crate::codec::CodecSize;
    use crate::codec::PropertyCodecSize;

    #[derive(CodecSize, PropertyCodecSize)]
    struct TestStruct {
        test_string: String,
        optional_string: Option<String>,
        optional_w_none: Option<String>,
        _a: u8,
        _b: Option<u16>,

        #[codec(skip_if = "Vec::is_empty", property_type = "PropertyType::TestProperty")]
        test_vec: Vec<u8>,
    }

    let test_instance = TestStruct {
        test_string: "hello".to_string(),           // 7 bytes (2 for length + 5 for "hello")
        optional_string: Some("world".to_string()), // 7 bytes (2 for length + 5 for "world")
        optional_w_none: None,                      // 0 bytes (None should result in size 0 for codec size)    
        _a: 1,                                      // 1 byte   
        _b: Some(2),                                // 2 bytes (size of u16)
        test_vec: vec![1, 2, 3],                    // 6 bytes - 1 byte for property identifier + 2 bytes for length prefix + 3 bytes for data    
    };
                                                    // 1 byte for property length

    let expected_size = 7 + 7 + 0 + 1 + 2 + 1 + (1 + 2 + 3); // size of test_string + optional_string + optianl_w_none + _a + _b + test_vec (property size wrapper + length prefix + data)
    assert_eq!(test_instance.codec_size(), expected_size);
}



#[test] 
fn test_complex_with_typed_size() {
    use crate::codec::CodecSize;
    use crate::codec::PropertyCodecSize;

    struct CustomType(u32);

    impl CodecSize for CustomType {
        fn codec_size(&self) -> u32 {
            4 // use the custom size function defined earlier
        }
     }

     impl PropertyCodecSize for CustomType {
         fn property_size(&self) -> u32 {
            5 // delegate to codec_size for property size as well
         }
     }


    #[derive(CodecSize, PropertyCodecSize)]
    struct TestStruct {
        test_string: String,
        optional_string: Option<String>,
        optional_w_none: Option<String>,
        _a: u8,
        _b: Option<u16>,

        #[codec(property_type = "PropertyType::TestProperty")]
        custom: CustomType,
    }

    let test_instance = TestStruct {
        test_string: "hello".to_string(),           // 7 bytes (2 for length + 5 for "hello")
        optional_string: Some("world".to_string()), // 7 bytes (2 for length + 5 for "world")
        optional_w_none: None,                      // 0 bytes (None should result in size 0 for codec size)    
        _a: 1,                                      // 1 byte   
        _b: Some(2),                                // 2 bytes (size of u16)
        custom: CustomType(3),                      // custom size of 5 bytes (1 byte for property identifier + 4 bytes for u32)    
    };                                              // 1 byte for property length
                                                

    let expected_size = 7 + 7 + 0 + 1 + 2 + 1 + 5; // size of test_string + optional_string + optianl_w_none + _a + _b + custom (custom size)
    assert_eq!(test_instance.property_size(), 5);
    assert_eq!(test_instance.codec_size(), expected_size);
}