use crate::buf::reader::Reader;

#[test]
fn test_read_var_int() {
    let encoded = [0x80, 0x01];
    let mut reader = Reader::new(&encoded[..], 2);
    let val = reader.read_variable_len_int();
    assert_eq!(128, val);
    // 777 --- 0x309
    let encoded = [0x89, 0x06];
    let mut reader = Reader::new(&encoded[..], 2);
    let val = reader.read_variable_len_int();
    assert_eq!(777, val);
    // 89_000 --- 0x15ba8
    let encoded = [0xA8, 0xB7, 0x05];
    let mut reader = Reader::new(&encoded[..], 3);
    let val = reader.read_variable_len_int();
    assert_eq!(89_000, val);
}
