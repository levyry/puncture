use std::io::Write;

#[expect(clippy::indexing_slicing, clippy::as_conversions)]
const CRC32_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut n: u32 = 0;
    while n < 256 {
        let mut c = n;
        let mut k: u8 = 0;
        while k < 8 {
            if (c & 1) == 1 {
                c = POLYNOMIAL ^ (c >> 1);
            } else {
                c >>= 1;
            }
            k = k.saturating_add(1);
        }
        table[n as usize] = c;
        n = n.saturating_add(1);
    }
    table
};

const POLYNOMIAL: u32 = 0xED_B8_83_20;

#[derive(Debug)]
pub struct Crc32<W> {
    stream: W,
    state: u32,
    isize: u32,
}

impl<W: Write> Crc32<W> {
    pub const fn new(stream: W) -> Self {
        Self {
            state: u32::MAX,
            stream,
            isize: 0,
        }
    }

    #[expect(clippy::indexing_slicing, clippy::as_conversions)]
    fn update(&mut self, buf: &[u8]) {
        for &byte in buf {
            let index_stub = self.state ^ u32::from(byte);
            let index = index_stub as usize & 0xFF;
            self.state = (self.state >> 8) ^ CRC32_TABLE[index];
        }
    }

    const fn finalize(&self) -> u32 {
        self.state ^ u32::MAX
    }

    pub const fn get_hashes(&self) -> (u32, u32) {
        (self.finalize(), self.isize)
    }
}

impl<W: Write> Write for Crc32<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.update(buf);
        self.isize = self
            .isize
            .wrapping_add(buf.len().try_into().expect("u32 couldn't fit in usize"));
        self.stream.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.stream.flush()
    }
}
