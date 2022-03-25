use std::io::{Read, Write};
use std::net::TcpStream;

const DEFAULT_PORT: u16 = 1883;
const DEFAULT_HOST: &'static str = "127.0.0.1";
const ECHO_MSG: &'static str = "Echo Test!";
const ECHO_MSG_LEN: usize = 10;

#[test]
/// This tests expects the basic TCP server to be running on
/// * Host: 127.0.0.1
/// * Port: 1883
fn test_echo() {
    match TcpStream::connect((DEFAULT_HOST, DEFAULT_PORT)) {
        Ok(mut stream) => {
            let mut buffer = [0u8; 128];
            match stream.write_all(ECHO_MSG.as_bytes()) {
                Ok(_) => match stream.read(&mut buffer) {
                    Ok(len) => {
                        assert_eq!(
                            ECHO_MSG_LEN, len,
                            "expected {} bytes in response",
                            ECHO_MSG_LEN
                        );
                        match String::from_utf8(buffer[0..ECHO_MSG_LEN].to_vec()) {
                            Ok(response) => assert_eq!(
                                ECHO_MSG.to_string(),
                                response,
                                "expected response to be {}",
                                ECHO_MSG
                            ),
                            Err(_) => {
                                assert!(false, "unable to convert response to valid UTF-8 string")
                            }
                        }
                    }
                    Err(e) => assert!(false, "unable to read echo message from broker: {}", e),
                },
                Err(e) => assert!(false, "unable to write echo message to broker: {}", e),
            }
        }
        Err(e) => assert!(false, "Unable to connect to test broker: {}", e.to_string()),
    }
}
