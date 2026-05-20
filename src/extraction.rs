use std::{
    ffi::CString,
    io::{BufRead, Write},
};

use anyhow::bail;

use crate::bitreader::BitReader;

pub struct Extraction<R, S> {
    data: BitReader<R>,
    state: S,
}

impl<R> Extraction<R, Start>
where
    R: BufRead,
{
    pub const fn new(data: R) -> Self {
        Self {
            data: BitReader::new(data),
            state: Start,
        }
    }

    pub fn process_header(mut self) -> anyhow::Result<Extraction<R, ProcessedHeader>> {
        let mut state = ProcessedHeader { file_name: None };

        let magic: u16 = self.data.read_bytes(2)?;
        if magic != 0x1F8B {
            bail!("Incorrect magic: {magic}");
        }

        let cm: u8 = self.data.read_bytes(1)?;
        if cm != 0x8 {
            bail!("Incorrect compression method: {cm}");
        }

        let flags: u8 = self.data.read_bytes(1)?;

        let fhcrc = (flags & 0x2) == 2;
        let fextra = (flags & 0x4) == 4;
        let fname = (flags & 0x8) == 8;
        let fcomment = (flags & 0x16) == 16;

        if flags < 0x20 {
            bail!("Flag reserved bits aren't zeroed out: {flags}");
        }

        // We skip MTIME, XFL and OS headers
        let _mtime: u32 = self.data.read_bytes(4)?;
        let _xfl_and_os: u16 = self.data.read_bytes(2)?;

        if fextra {
            let xlen: u16 = self.data.read_bytes(2)?;
            self.data.skip_bytes(xlen.into())?;
        }

        if fname {
            let mut name: Vec<u8> = vec![];
            name.push(self.data.read_bytes(1)?);
            while name.last() != Some(&0x00) {
                name.push(self.data.read_bytes(1)?);
            }
            state.file_name = Some(CString::from_vec_with_nul(name)?);
        }

        if fcomment {
            let mut byte: u8 = self.data.read_bytes(1)?;
            while byte != 0x00 {
                byte = self.data.read_bytes(1)?;
            }
        }

        // TODO: Currently, the crc16 field is ignored if it exists.
        // I could calculate this, but then I would need to keep a
        // seperate buffer for all the header fields I read in.
        let mut _crc16: Option<u16> = if fhcrc {
            Some(self.data.read_bytes(2)?)
        } else {
            None
        };

        Ok(Extraction {
            data: self.data,
            state,
        })
    }
}

impl<R> Extraction<R, ProcessedHeader>
where
    R: BufRead,
{
    pub const fn get_file_name(&self) -> Option<&CString> {
        self.state.file_name.as_ref()
    }

    pub fn extract_into(
        mut self,
        output: &mut impl Write,
    ) -> anyhow::Result<Extraction<R, Finish>> {
        let mut bfinal: u8 = self.data.read_bits(1)?;

        while bfinal == 0 {
            let btype: u8 = self.data.read_bits(2)?;

            match btype {
                0b00 => self.uncompressed_data(output),
                0b01 => self.fixed_huffman(output),
                0b10 => self.dynamic_huffman(output),
                0b11 => bail!("Hit reserved Huffman btype header: 11"),
                _ => unreachable!("We only read two bits"),
            }

            bfinal = self.data.read_bits(1)?;
        }

        // If the while breaks, that means the first bit of the deflate header
        // was set, so this is the last block for this member
        let btype: u16 = self.data.read_bits(2)?;

        match btype {
            0b00 => self.uncompressed_data(output),
            0b01 => self.fixed_huffman(output),
            0b10 => self.dynamic_huffman(output),
            0b11 => bail!("Hit reserved Huffman btype header: 11"),
            _ => unreachable!("We only read two bits"),
        }

        let crc32: u32 = self.data.read_bytes(4)?;
        let isize: u32 = self.data.read_bytes(4)?;

        self.check_crc32(crc32, isize);

        Ok(Extraction {
            data: self.data,
            state: Finish,
        })
    }

    fn uncompressed_data(&mut self, _output: &mut impl Write) {
        todo!()
    }

    fn fixed_huffman(&mut self, _output: &mut impl Write) {
        todo!()
    }

    fn dynamic_huffman(&mut self, _output: &mut impl Write) {
        todo!()
    }

    fn check_crc32(&self, _crc32: u32, _isize: u32) {
        todo!()
    }
}

// State management stuff
pub struct Start;
pub struct ProcessedHeader {
    file_name: Option<CString>,
}
pub struct Finish;

#[allow(dead_code, reason = "This is only for the typestate pattern")]
pub trait ExtractionState {}
impl ExtractionState for Start {}
impl ExtractionState for ProcessedHeader {}
impl ExtractionState for Finish {}
