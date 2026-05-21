use std::{
    ffi::CString,
    io::{BufRead, Write},
};

use anyhow::{Context, Result, bail};

use crate::{bitreader::BitReader, crc32::Crc32};

const GZIP_MAGIC: [u8; 2] = [0x1F, 0x8B];
const CM_DEFLATE: u8 = 8;

#[derive(Debug)]
pub struct Extractor<'a, R> {
    data: &'a mut BitReader<R>,
    file_name: Option<CString>,
    running_crc32: Crc32,
    running_isize: u32,
}

impl<'a, R: BufRead> Extractor<'a, R> {
    pub const fn new(data: &'a mut BitReader<R>) -> Self {
        Self {
            data,
            file_name: None,
            running_crc32: Crc32::new(),
            running_isize: 0,
        }
    }

    pub const fn get_file_name(&self) -> Option<&CString> {
        self.file_name.as_ref()
    }

    pub fn process_header(&mut self) -> Result<()> {
        let mut magic = [0; 2];
        self.data.read_raw_bytes(&mut magic)?;

        if magic != GZIP_MAGIC {
            bail!("Incorrect magic: {}{}", magic[0], magic[1]);
        }

        let cm: u8 = self.data.read_bytes(1)?;
        if cm != CM_DEFLATE {
            bail!("Incorrect compression method: {cm}");
        }

        let flags: u8 = self.data.read_bytes(1)?;

        let fhcrc = (flags & 0x02) != 0;
        let fextra = (flags & 0x04) != 0;
        let fname = (flags & 0x08) != 0;
        let fcomment = (flags & 0x10) != 0;

        if flags & 0xE0 != 0 {
            bail!("Flag reserved bits aren't zeroed out: {flags}");
        }

        // TODO: We skip MTIME, XFL and OS headers.
        let _mtime: u32 = self.data.read_bytes(4)?;
        let _xfl_and_os: u16 = self.data.read_bytes(2)?;

        if fextra {
            let xlen: u16 = self.data.read_bytes(2)?;
            self.data.skip_bytes(xlen.into())?;
        }

        if fname {
            let mut name = Vec::new();

            loop {
                let mut byte = [0; 1];
                self.data.read_raw_bytes(&mut byte)?;
                name.push(byte[0]);

                if byte[0] == 0x00 {
                    break;
                }
            }
            self.file_name = Some(CString::from_vec_with_nul(name)?);
        }

        if fcomment {
            loop {
                let mut byte = [0; 1];
                self.data.read_raw_bytes(&mut byte)?;
                if byte[0] == 0x00 {
                    break;
                }
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

    pub fn deflate(mut self, output: &mut impl Write) -> Result<()> {
        loop {
            let bfinal: u8 = self.data.read_bits(1)?;
            let btype: u8 = self.data.read_bits(2)?;

            match btype {
                0b00 => self.uncompressed_data(output)?,
                0b01 => self.fixed_huffman(output)?,
                0b10 => self.dynamic_huffman(output)?,
                0b11 => bail!("Hit reserved Huffman btype header: 11"),
                _ => unreachable!("We only read two bits"),
            }

            if bfinal != 0 {
                break;
            }
        }

        let expected_crc32: u32 = self.data.read_bytes(4)?;
        let expected_isize: u32 = self.data.read_bytes(4)?;

        let calculated_crc = self.running_crc32.finalize();
        let calculated_isize = self.running_isize;

        if calculated_crc != expected_crc32 {
            bail!(
                "Calculated crc32 hash ({calculated_crc}) doesn't match expected ({expected_crc32})."
            );
        }

        if calculated_isize != expected_isize {
            bail!("Payload size ({calculated_isize}) doesn't match expected ({expected_isize}).")
        }

        Ok(())
    }

    fn uncompressed_data(&mut self, output: &mut impl Write) -> Result<()> {
        self.data.align_to_byte();
        let len: u16 = self.data.read_bytes(2)?;
        let nlen: u16 = self.data.read_bytes(2)?;

        if len != !nlen {
            bail!("Member nlen isn't one's complement of len. len: {len}, nlen: {nlen}");
        }

        let mut payload = vec![0u8; len.into()];

        self.data.read_raw_bytes(&mut payload)?;

        self.running_crc32.update(&payload);
        self.running_isize = self.running_isize.wrapping_add(
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
