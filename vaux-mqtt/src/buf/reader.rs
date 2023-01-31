pub struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
    len: usize,
}

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8], len: usize) -> Self {
        Reader { buf, pos: 0, len }
    }

    /// Reads an MQTT variable length unsigned integer value into a u32.
    pub fn read_variable_len_int(&mut self) -> u32 {
        let mut result = 0_u32;
        let mut shift = 0;
        let mut next_byte = self.buf[self.pos];
        self.pos += 1;
        if self.pos >= self.len {}
        let mut decode = true;
        while decode {
            result += ((next_byte & 0x7f) as u32) << shift;
            shift += 7;
            if next_byte & 0x80 == 0 {
                decode = false;
            } else {
                next_byte = self.buf[self.pos];
                self.pos += 1;
                if self.pos >= self.len {}
            }
        }
        result
    }
}
