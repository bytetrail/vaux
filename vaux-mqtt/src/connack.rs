
#[repr(u8)]
#[derive(Debug, Eq, PartialEq)]
pub enum Reason {
    Success,
    UnspecifiedErr = 0x80,
    MalformedPacket,
    ProtocolErr,
    ImplementationErr,
    UnsupportedProtocolVersion,
    InvalidClientId,
    AuthenticationErr,
    Unauthorized,
    ServerUnavailable,
    ServerBusy,
    Banned,
    AuthMethodErr = 0x8c,
    InvalidTopicName = 0x90,
    PacketTooLarge = 0x95,
    QuotaExceeded = 0x97,
    PayloadFormatErr = 0x99,
    RetainNotSupported,
    QoSNotSupported,
    UseDiffServer,
    ServerMoved,
    ConnRateExceeded = 0x9f
}



pub struct ConnAck {
    session_present: bool,
    reason: Reason,
}

impl crate::Sized for ConnAck {
    fn size(&self) -> usize {
        let mut size = 0;
        // minimum size of ack flags and reason code
        size = 2;
        size
    }
}