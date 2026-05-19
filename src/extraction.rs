use std::io::{self, BufRead, Write};

use crate::bitreader::BitReader;

pub struct Extraction<'a, R: BufRead, W: Write> {
    data: BitReader<R>,
    output: &'a mut W,
}

impl<'a, R: BufRead, W: Write> Extraction<'a, R, W> {
    pub const fn new(data: R, output: &'a mut W) -> Self {
        Self {
            data: BitReader::new(data),
            output,
        }
    }

    pub fn extract(&mut self) {
        loop {
            let _result = self.extract_member();
        }
    }

    fn extract_member(&mut self) -> io::Result<()> {
        let magic = self.data.read_bytes(2)?;
        assert!(magic == 0x1F8B, "Incorrect magic.");

        let cm = self.data.read_bytes(1)?;
        assert!(cm == 0x8, "Incorrect compression method.");

        let flags = self.data.read_bytes(1)?;

        let fhcrc = (flags & 0x2) == 1;
        let fextra = (flags & 0x4) == 1;
        let fname = (flags & 0x8) == 1;
        let fcomment = (flags & 0x16) == 1;
        assert!(flags < 0x20, "Flag reserved bits not 0.");

        // We skip MTIME, XFL and OS headers
        let _skipped_header = self.data.read_bytes(4)?;
        let _skipped_header = self.data.read_bytes(2)?;

        if fextra {
            let xlen_bytes = self.data.read_bytes(2)?;
            let _xlen: u16 = xlen_bytes.try_into().expect("We just read two bytes");
        }

        if fname {
            let mut byte = self.data.read_bytes(1)?;
            while byte != 0x00 {
                byte = self.data.read_bytes(1)?;
            }
        }

        if fcomment {
            let mut byte = self.data.read_bytes(1)?;
            while byte != 0x00 {
                byte = self.data.read_bytes(1)?;
            }
        }

        let _crc16: Option<u16> = if fhcrc {
            let crc16_bytes = self.data.read_bytes(2)?;
            Some(crc16_bytes.try_into().expect("We just read two bytes"))
        } else {
            None
        };

        // Compressed blocks
        let (crc32, isize) = self.deflate()?;

        self.check_crc32(crc32, isize);

        Ok(())
    }

    fn deflate(&mut self) -> io::Result<(u32, u32)> {
        let mut bfinal = self.data.read_bits(1)?;

        while bfinal == 0 {
            let btype = self.data.read_bits(2)?;

            match btype {
                0b00 => self.uncompressed_data(),
                0b01 => self.fixed_huffman(),
                0b10 => self.dynamic_huffman(),
                0b11 => panic!("Hit reserved huffman header"),
                _ => unreachable!("We only read two bits"),
            }

            bfinal = self.data.read_bits(1)?;
        }

        // If the while breaks, that means the first bit of the deflate header
        // was set, so this is the last block for this member
        let btype = self.data.read_bits(2)?;

        match btype {
            0b00 => self.uncompressed_data(),
            0b01 => self.fixed_huffman(),
            0b10 => self.dynamic_huffman(),
            0b11 => panic!("Hit reserved huffman header"),
            _ => unreachable!("We only read two bits"),
        }

        let crc32: u32 = self
            .data
            .read_bytes(4)?
            .try_into()
            .expect("We just read four bytes");

        let isize: u32 = self
            .data
            .read_bytes(4)?
            .try_into()
            .expect("We just read four bytes");

        Ok((crc32, isize))
    }

    fn uncompressed_data(&self) {
        todo!()
    }

    fn fixed_huffman(&self) {
        todo!()
    }

    fn dynamic_huffman(&self) {
        todo!()
    }

    fn check_crc32(&self, _crc32: u32, _isize: u32) {
        todo!()
    }
}
