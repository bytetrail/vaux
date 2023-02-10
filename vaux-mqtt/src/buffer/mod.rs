use crate::MQTTCodecError;

pub struct Buffer<'a> {
    data: &'a mut Vec<u8>,
    pos: usize,
}

impl<'a> Buffer<'a> {
    pub fn new(data: &'a mut Vec<u8>) -> Self {
        Self { data: data, pos: 0 }
    }

    /// Gets a boolean value from the buffer and advances the position
    pub fn get_bool(&mut self) -> bool {
        assert!(self.pos + 1 <= self.data.len());
        self.pos += 1;
        self.data[self.pos - 1] != 0
    }

    pub fn get_u8(&mut self) -> u8 {
        assert!(self.pos + 1 <= self.data.len());
        self.pos += 1;
        self.data[self.pos - 1]
    }

    pub fn get_u16(&mut self) -> u16 {
        assert!(self.pos + 2 <= self.data.len());
        self.pos += 2;
        (self.data[self.pos - 2] as u16) << 8 | self.data[self.pos - 1] as u16
    }

    pub fn get_u32(&mut self) -> u32 {
        assert!(self.pos + 4 <= self.data.len());
        self.pos += 4;
        (self.data[self.pos - 4] as u32) << 24
            | (self.data[self.pos - 3] as u32) << 16
            | (self.data[self.pos - 2] as u32) << 8
            | self.data[self.pos - 1] as u32
    }

    pub fn get_var_u32(&mut self) -> u32 {
        let mut result = 0_u32;
        let mut shift = 0;
        let mut next_byte = self.get_u8();
        let mut decode = true;
        while decode {
            result += ((next_byte & 0x7f) as u32) << shift;
            shift += 7;
            if next_byte & 0x80 == 0 {
                decode = false;
            } else {
                next_byte = self.get_u8();
            }
        }
        result
    }

    pub fn get_bin(&mut self) -> Result<Vec<u8>, MQTTCodecError> {
        let len = self.get_u16() as usize;
        if self.pos + len > self.data.len() {
            return Err(MQTTCodecError::new("insufficient data in buffer"));
        }
        let prev_pos = self.pos;
        self.pos += len;
        Ok(Vec::from(&self.data[prev_pos..self.pos]))
    }

    pub fn get_bin_as_ref(&mut self) -> Result<&[u8], MQTTCodecError> {
        let len = self.get_u16() as usize;
        if self.pos + len > self.data.len() {
            return Err(MQTTCodecError::new("insufficient data in buffer"));
        }
        let prev_pos = self.pos;
        self.pos += len;
        Ok(&self.data[prev_pos..self.pos])
    }

    pub fn get_utf8(&mut self) -> Result<String, MQTTCodecError> {
        let str_len = self.get_u16() as usize;
        let prev_pos = self.pos;
        self.pos += str_len;
        if let Ok(s) = std::str::from_utf8(&self.data[prev_pos..self.pos]) {
            Ok(s.to_owned())
        } else {
            println!("Invalid UTF 8 characters");
            Err(MQTTCodecError::new("invalid UTF8 characters in string"))
        }
    }

    pub fn get_utf8_as_ref(&mut self) -> Result<&str, MQTTCodecError> {
        let str_len = self.get_u16() as usize;
        let prev_pos = self.pos;
        self.pos += str_len;
        if let Ok(s) = std::str::from_utf8(&self.data[prev_pos..self.pos]) {
            Ok(s)
        } else {
            println!("Invalid UTF 8 characters");
            Err(MQTTCodecError::new("invalid UTF8 characters in string"))
        }
    }

    pub fn put_bool(&mut self, val: bool) -> usize {
        self.data[self.pos] = val as u8;
        self.pos += 1;
        self.pos
    }

    pub fn put_u8(&mut self, val: u8) -> usize {
        self.data[self.pos] = val;
        self.pos += 1;
        self.pos
    }

    pub fn put_u16(&mut self, val: u16) -> usize {
        self.data[self.pos] = ((val & 0xff_00) >> 8) as u8;
        self.pos += 1;
        self.data[self.pos] = (val & 0x00_ff) as u8;
        self.pos += 1;
        self.pos
    }

    pub fn put_u32(&mut self, val: u32) -> usize {
        let mut mask = 0x_ff_00_00_00_u32;
        for i in (0..4).rev() {
            self.data[self.pos] = ((val & mask) >> (8 * i)) as u8;
            mask >>= 8;
            self.pos += 1;
        }
        self.pos
    }
}
