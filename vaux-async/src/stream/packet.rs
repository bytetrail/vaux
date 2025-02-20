use super::AsyncMqttStream;
use bytes::{Bytes, BytesMut};
use std::fmt::Display;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use vaux_mqtt::{decode, encode, Packet};

const READ_BUFFER_SIZE: usize = 4096;
const MAX_BUFFER_SIZE: usize = 4096 * 1024;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    ReadTimeout(std::io::Error),
    ReadBuffer,
    Codec(vaux_mqtt::codec::MqttCodecError),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "IO error: {}", e),
            Error::ReadTimeout(e) => write!(f, "Read timeout: {}", e),
            Error::ReadBuffer => write!(f, "Read buffer full"),
            Error::Codec(e) => write!(f, "MQTT codec error: {}", e),
        }
    }
}

pub struct PacketStream {
    read_buffer: Vec<u8>,
    read_offset: usize,
    initial_buffer_size: usize,
    max_buffer_size: usize,
    stream: AsyncMqttStream,
}

impl PacketStream {
    pub fn new(
        stream: AsyncMqttStream,
        initial_buffer_size: Option<usize>,
        max_buffer_size: Option<usize>,
    ) -> Self {
        Self {
            read_buffer: vec![0; initial_buffer_size.unwrap_or(READ_BUFFER_SIZE)],
            read_offset: 0,
            initial_buffer_size: initial_buffer_size.unwrap_or(READ_BUFFER_SIZE),
            max_buffer_size: max_buffer_size.unwrap_or(MAX_BUFFER_SIZE),
            stream,
        }
    }

    /// Returns the size of the read buffer. This is the maximum number of bytes that can be
    /// read from the stream before the buffer is full and may need to be resized for continued
    /// reading. The buffer is resized by doubling the size of the buffer when it is full.
    ///
    pub fn buffer_size(&self) -> usize {
        self.read_buffer.len()
    }

    /// Reads the next packet from the stream. This method will block until a packet is
    /// available or an error is encountered reading from the stream or decoding the
    /// packet.
    ///
    /// Cancel safe. No data is lost if the task is cancelled at the await point. The
    /// stream will be read again on the next call to this method and any bytes not
    /// decoded will be retained.
    ///
    ///
    pub async fn read(&mut self) -> Result<Option<Packet>, Error> {
        let result = self.read_internal(false).await;
        match result {
            Ok(Some((packet, _))) => Ok(Some(packet)),
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub async fn read_raw(&mut self) -> Result<Option<(Packet, Bytes)>, Error> {
        let result = self.read_internal(true).await;
        match result {
            Ok(Some((packet, Some(bytes)))) => Ok(Some((packet, bytes))),
            // this would be an error to get a packet without bytes when raw is requested
            Ok(Some((_, None))) => Err(Error::ReadBuffer),
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn read_internal(
        &mut self,
        with_bytes: bool,
    ) -> Result<Option<(Packet, Option<Bytes>)>, Error> {
        let mut bytes_read = self.read_offset;
        loop {
            if bytes_read > 0 {
                let bytes_mut = &mut BytesMut::from(&self.read_buffer[0..bytes_read]);
                match decode(bytes_mut) {
                    Ok(data_read) => {
                        if let Some((packet, decode_len)) = data_read {
                            if with_bytes {
                                // split the buffer into packet and remaining bytes
                                let packet_bytes = bytes_mut.split_to(decode_len as usize);
                                // resize the buffer if less than requested initial size
                                if self.read_buffer.len() < self.initial_buffer_size {
                                    self.read_buffer.resize(self.initial_buffer_size, 0);
                                }
                                return Ok(Some((packet, Some(packet_bytes.freeze()))));
                            } else {
                                if decode_len < bytes_read as u32 {
                                    self.read_buffer
                                        .copy_within(decode_len as usize..bytes_read, 0);
                                    // adjust offset to end of decoded bytes
                                    self.read_offset = bytes_read - decode_len as usize;
                                } else {
                                    self.read_offset = 0;
                                }
                                return Ok(Some((packet, None)));
                            }
                        } else {
                            return Ok(None);
                        }
                    }
                    Err(e) => match &e.kind {
                        vaux_mqtt::codec::ErrorKind::InsufficientData(_expected, _actual) => {
                            // fail when read buffer space if fully allocated
                            if self.read_offset >= self.max_buffer_size {
                                return Err(Error::ReadBuffer);
                            }
                            // fall through the the socket read
                        }
                        _ => {
                            return Err(Error::Codec(e));
                        }
                    },
                }
            }
            match self
                .stream
                .read(&mut self.read_buffer[self.read_offset..])
                .await
            {
                Ok(len) => {
                    if len == 0 && bytes_read == 0 {
                        return Ok(None);
                    }
                    bytes_read += len;
                    self.read_offset = bytes_read;
                    if self.read_offset >= self.read_buffer.len() {
                        // increase buffer size
                        // TODO add a capability to shrink the buffer if it is too large or
                        // a client needs to free up memory
                        let new_size = usize::min(self.read_buffer.len() * 2, self.max_buffer_size);
                        self.read_buffer.resize(new_size, 0);
                    }
                }
                Err(e) => match e.kind() {
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut => {
                        return Err(Error::ReadTimeout(e));
                    }
                    _ => {
                        return Err(Error::Io(e));
                    }
                },
            }
        }
    }

    pub async fn write(&mut self, packet: &Packet) -> Result<(), Error> {
        let mut dest = BytesMut::default();
        let result = encode(&packet, &mut dest);
        if let Err(e) = result {
            return Err(Error::Codec(e));
        }
        if let Err(e) = self.stream.write_all(&dest).await {
            return Err(Error::Io(e));
        }
        Ok(())
    }

    pub async fn shutdown(&mut self) -> Result<(), Error> {
        if let Err(e) = self.stream.shutdown().await {
            return Err(Error::Io(e));
        }
        Ok(())
    }
}
