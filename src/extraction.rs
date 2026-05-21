use std::{
    ffi::CString,
    io::{BufRead, Write},
};

use anyhow::{Context, Result, bail};

use crate::{bitreader::BitReader, crc32::Crc32};

pub struct Extraction<'a, R> {
    data: &'a mut BitReader<R>,
    state: ExtractionState,
}

impl<'a, R> Extraction<'a, R>
where
    R: BufRead,
{
    pub fn new(data: &'a mut BitReader<R>) -> Self {
        Self {
            data,
            state: ExtractionState::default(),
        }
    }

    pub fn process_header(&mut self) -> Result<()> {
        let mut magic = [0; 2];
        self.data.read_raw_bytes(&mut magic)?;

        if magic != [0x1F, 0x8B] {
            bail!("Incorrect magic: {}{}", magic[0], magic[1]);
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

        if flags >= 0x20 {
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
            let mut byte = [0; 1];
            self.data.read_raw_bytes(&mut byte)?;
            name.push(byte[0]);
            while name.last() != Some(&0x00) {
                let mut byte = [0; 1];
                self.data.read_raw_bytes(&mut byte)?;
                name.push(byte[0]);
            }
            self.state.file_name = Some(CString::from_vec_with_nul(name)?);
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

        Ok(())
    }

    pub const fn get_file_name(&self) -> Option<&CString> {
        self.state.file_name.as_ref()
    }

    pub fn deflate(mut self, output: &mut impl Write) -> Result<()> {
        let mut bfinal: u8 = self.data.read_bits(1)?;

        while bfinal == 0 {
            let btype: u8 = self.data.read_bits(2)?;

            match btype {
                0b00 => self.uncompressed_data(output)?,
                0b01 => self.fixed_huffman(output)?,
                0b10 => self.dynamic_huffman(output)?,
                0b11 => bail!("Hit reserved Huffman btype header: 11"),
                _ => unreachable!("We only read two bits"),
            }

            bfinal = self.data.read_bits(1)?;
        }

        // If the while breaks, that means the first bit of the deflate header
        // was set, so this is the last block for this member
        let btype: u16 = self.data.read_bits(2)?;

        match btype {
            0b00 => self.uncompressed_data(output)?,
            0b01 => self.fixed_huffman(output)?,
            0b10 => self.dynamic_huffman(output)?,
            0b11 => bail!("Hit reserved Huffman btype header: 11"),
            _ => unreachable!("We only read two bits"),
        }

        let expected_crc32: u32 = self.data.read_bytes(4)?;
        let expected_isize: u32 = self.data.read_bytes(4)?;

        let calculated_crc = self.state.running_crc32.finalize();
        let calculated_isize = self.state.running_isize;

        if calculated_crc != expected_crc32 {
            bail!("CRC32 doesn't match. Payload was corrupted.");
        }

        if calculated_isize != expected_isize {
            bail!("Payload size doesn't match expected.")
        }

        Ok(())
    }

    fn uncompressed_data(&mut self, output: &mut impl Write) -> Result<()> {
        self.data.align_to_byte();
        let len: u16 = self.data.read_bytes(2)?;
        let nlen: u16 = self.data.read_bytes(2)?;

        if !nlen != len {
            bail!("Member nlen isn't one's complement of len. len: {len}, nlen: {nlen}");
        }

        let mut payload = vec![0u8; len.into()];

        self.data.read_raw_bytes(&mut payload)?;

        self.state.running_crc32.update(&payload);
        self.state.running_isize = self.state.running_isize.wrapping_add(
            payload
                .len()
                .try_into()
                .context("Couldn't fit member byte count in u32")?,
        );

        output.write_all(&payload)?;

        Ok(())
    }

    fn fixed_huffman(&mut self, _output: &mut impl Write) -> Result<()> {
        todo!()
    }

    fn dynamic_huffman(&mut self, _output: &mut impl Write) -> Result<()> {
        todo!()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ExtractionState {
    file_name: Option<CString>,
    running_crc32: Crc32,
    running_isize: u32,
}
