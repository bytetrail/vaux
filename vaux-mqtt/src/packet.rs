use crate::fixed::FixedHeader;
use crate::{CodecSize, Decode, Encode, MqttCodecError, PacketType, PropertyCodecSize};
use bytes::BytesMut;

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct Empty;

impl CodecSize for Empty {
    fn codec_size(&self) -> u32 {
        0
    }
}

impl PropertyCodecSize for Empty {
    fn property_size(&self) -> u32 {
        0
    }
}

impl Decode for Empty {
    fn decode(&mut self, _src: &mut BytesMut) -> Result<(), MqttCodecError> {
        Ok(())
    }
}
impl Encode for Empty {
    fn encode(&mut self, _dest: &mut BytesMut) -> Result<(), MqttCodecError> {
        Ok(())
    }
}

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct ControlPacket<H, P>
where
    H: Decode + Encode + Clone + PartialEq + Eq + CodecSize,
    P: Decode + Encode + Clone + PartialEq + Eq + CodecSize,
{
    pub(crate) fixed_header: FixedHeader,
    pub(crate) variable_header: H,
    pub(crate) payload: P,
}

impl<H, P> Decode for ControlPacket<H, P>
where
    H: Decode + Encode + Clone + PartialEq + Eq + CodecSize,
    P: Decode + Encode + Clone + PartialEq + Eq + CodecSize,
{
    fn decode(&mut self, src: &mut BytesMut) -> Result<(), MqttCodecError> {
        self.variable_header.decode(src)?;
        self.payload.decode(src)?;
        Ok(())
    }
}

impl<H, P> Encode for ControlPacket<H, P>
where
    H: Decode + Encode + Clone + PartialEq + Eq + PropertyCodecSize + CodecSize,
    P: Decode + Encode + Clone + PartialEq + Eq + CodecSize,
{
    fn encode(&mut self, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
        self.fixed_header
            .set_remaining(self.variable_header.codec_size() + self.payload.codec_size());
        self.fixed_header.encode(dest)?;
        self.variable_header.encode(dest)?;
        self.payload.encode(dest)?;
        Ok(())
    }
}

impl<H, P> PropertyCodecSize for ControlPacket<H, P>
where
    H: Decode + Encode + Clone + PartialEq + Eq + PropertyCodecSize + CodecSize,
    P: Decode + Encode + Clone + PartialEq + Eq + CodecSize,
{
    fn property_size(&self) -> u32 {
        self.variable_header.property_size()
    }
}

impl<H, P> CodecSize for ControlPacket<H, P>
where
    H: Default + Decode + Encode + Clone + PartialEq + Eq + PropertyCodecSize + CodecSize,
    P: Default + Decode + Encode + Clone + PartialEq + Eq + CodecSize,
{
    fn codec_size(&self) -> u32 {
        self.variable_header.codec_size() + self.payload.codec_size()
    }
}

impl<H, P> ControlPacket<H, P>
where
    H: Default + Decode + Encode + Clone + PartialEq + Eq + PropertyCodecSize + CodecSize,
    P: Default + Decode + Encode + Clone + PartialEq + Eq + CodecSize,
{
    pub fn new(fixed_header: FixedHeader) -> Self {
        ControlPacket {
            fixed_header,
            variable_header: H::default(),
            payload: P::default(),
        }
    }

    pub fn new_with_type(packet_type: PacketType) -> Self {
        let fixed_header = FixedHeader::new(packet_type);
        ControlPacket {
            fixed_header,
            variable_header: H::default(),
            payload: P::default(),
        }
    }

    pub fn header(&self) -> &H {
        &self.variable_header
    }

    pub fn header_mut(&mut self) -> &mut H {
        &mut self.variable_header
    }

    pub fn payload(&self) -> &P {
        &self.payload
    }

    pub fn payload_mut(&mut self) -> &mut P {
        &mut self.payload
    }

    pub fn set_payload(&mut self, payload: P) {
        self.payload = payload;
    }
}
