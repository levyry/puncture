const CRC32_TABLE: [u32; 256] = Crc32::make_crc_table();

pub struct Crc32 {
    state: u32,
}

impl Crc32 {
    #[expect(clippy::indexing_slicing, reason = "n is kept < 256 by the while loop")]
    #[expect(
        clippy::cast_possible_truncation,
        reason = "n is kept < 256 by the while loop"
    )]
    #[expect(clippy::as_conversions, reason = "n is kept < 256 by the while loop")]
    const fn make_crc_table() -> [u32; 256] {
        let mut table = [0u32; 256];
        let mut n: usize = 0;
        while n < 256 {
            let mut c = n as u32;
            let mut k: u8 = 0;
            while k < 8 {
                if (c & 1) == 1 {
                    c = 0xED_B8_83_20 ^ (c >> 1);
                } else {
                    c >>= 1;
                }
                k = k.saturating_add(1);
            }
            table[n] = c;
            n = n.saturating_add(1);
        }
        table
    }

    pub const fn new() -> Self {
        Self {
            state: 0xFF_FF_FF_FF,
        }
    }

    #[expect(
        clippy::as_conversions,
        reason = "the as casts here only pad with zeros"
    )]
    #[expect(
        clippy::indexing_slicing,
        reason = "Bottom 8 bits of `byte` are masked"
    )]
    pub fn update(&mut self, buf: &[u8]) {
        for &byte in buf {
            let index = ((self.state ^ (u32::from(byte))) & 0xFF) as usize;
            self.state = (self.state >> 8) ^ CRC32_TABLE[index];
        }
    }

    pub const fn finalize(&self) -> u32 {
        self.state ^ 0xFF_FF_FF_FF
    }
}
