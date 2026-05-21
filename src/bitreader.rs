use std::io::BufRead;

use anyhow::{Context, Result, bail};

/// A reader that can extract bits LSB first.
pub struct BitReader<R> {
    data: R,
    bit_store: u128,
    num_of_stored_bits: u8,
}

impl<R: BufRead> BitReader<R> {
    pub const fn new(data: R) -> Self {
        Self {
            data,
            bit_store: 0,
            num_of_stored_bits: 0,
        }
    }

    /// Read at most 64 bits at a time.
    pub fn read_bits<T>(&mut self, num_of_bits: u8) -> Result<T>
    where
        T: TryFrom<u128>,
    {
        // Read until we have enough bits
        while self.num_of_stored_bits < num_of_bits {
            let mut buf = [0; 1];
            self.data
                .read_exact(&mut buf)
                .context("Hit EOF while filling buffer for requested bits")?;

            let scratch = u8::from_le_bytes(buf);
            let scratch: u128 = scratch.into();

            self.bit_store |= scratch << self.num_of_stored_bits;
            self.num_of_stored_bits = self
                .num_of_stored_bits
                .checked_add(8)
                .context("Tried storing too many bits in BitReader")?;
        }

        let mask: u128 = 1 << num_of_bits;
        let mask = mask.saturating_sub(1);

        let result = (self.bit_store & mask)
            .try_into()
            .map_err(|_| unreachable!());

        // Clear out internal state
        self.bit_store >>= num_of_bits;
        self.num_of_stored_bits = self.num_of_stored_bits.saturating_sub(num_of_bits);

        result
    }

    /// Read at most 8 bytes at a time.
    pub fn read_bytes<T>(&mut self, num_of_bytes: u8) -> Result<T>
    where
        T: TryFrom<u128>,
    {
        let bits = num_of_bytes.saturating_mul(8);

        if bits > 64 {
            bail!("Tried getting too many bytes from BitReader");
        }

        self.read_bits(bits)
    }

    /// Discards any remaining bits in the current byte to align with the next
    /// byte boundary.
    #[inline]
    pub const fn align_to_byte(&mut self) {
        let leftover_bits = self.num_of_stored_bits % 8;
        if leftover_bits > 0 {
            self.bit_store >>= leftover_bits;
            self.num_of_stored_bits = self.num_of_stored_bits.saturating_sub(leftover_bits);
        }
    }

    /// Skip any number of bytes. The skipped bytes will be discarded.
    pub fn skip_bytes(&mut self, num_of_bytes: u64) -> Result<()> {
        let cycles = num_of_bytes / 4;
        let rem: u8 = (num_of_bytes % 4)
            .try_into()
            .context("Since x mod 4 can only be 0..=3, this will always fit in a u8,")?;

        for _ in 0..cycles {
            self.read_bits::<u32>(32)?;
        }

        self.read_bits::<u32>(rem.saturating_mul(8))?;

        Ok(())
    }

    /// Reads raw bytes exactly as they appear in the stream, preserving order.
    /// Used for magic numbers, strings, and uncompressed block payloads.
    pub fn read_raw_bytes(&mut self, buf: &mut [u8]) -> Result<()> {
        for byte in buf.iter_mut() {
            if self.num_of_stored_bits >= 8 {
                *byte = (self.bit_store & 0xFF)
                    .try_into()
                    .context("We masked for exactly 8 bits")?;

                self.bit_store >>= 8;
                self.num_of_stored_bits = self.num_of_stored_bits.saturating_sub(8);
            } else {
                let mut temp = [0u8; 1];
                self.data
                    .read_exact(&mut temp)
                    .context("Hit EOF while reading raw bytes")?;
                *byte = temp[0];
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![expect(clippy::panic_in_result_fn, reason = "Using assert in tests")]
    use super::*;

    use std::io::Cursor;

    fn create_reader(bytes: &[u8]) -> BitReader<Cursor<Vec<u8>>> {
        BitReader::new(Cursor::new(bytes.to_vec()))
    }

    #[test]
    fn test_read_basic_bits() -> anyhow::Result<()> {
        // [0xCA] -> [1100_1010]
        let mut br = create_reader(&[0b1100_1010]);

        let bits1: u8 = br.read_bits(4)?;
        assert_eq!(bits1, 0b1010);

        let bits2: u8 = br.read_bits(4)?;
        assert_eq!(bits2, 0b1100);

        Ok(())
    }

    #[test]
    fn test_cross_byte_boundary() -> anyhow::Result<()> {
        // [0x33, 0x55] -> [0011_0011, 0101_0101]
        let mut br = create_reader(&[0x33, 0x55]);

        // Combined LSB first: 0101_0011_0011 -> 0x533
        let bits: u16 = br.read_bits(12)?;
        assert_eq!(bits, 0x533);

        let remaining: u8 = br.read_bits(4)?;
        assert_eq!(remaining, 0b0101);

        Ok(())
    }

    #[test]
    fn test_align_to_byte() -> anyhow::Result<()> {
        // [0xFF, 0xAA] -> [1111_1111, 1010_1010]
        let mut br = create_reader(&[0xFF, 0xAA]);

        let _: u8 = br.read_bits(3)?;
        br.align_to_byte();

        let next_byte: u8 = br.read_bits(8)?;
        assert_eq!(next_byte, 0xAA);

        Ok(())
    }

    #[test]
    fn test_read_dynamic_bytes() -> anyhow::Result<()> {
        let mut br = create_reader(&[0xAA, 0xBB, 0xCC, 0xDD]);

        let val: u32 = br.read_bytes(3)?;
        assert_eq!(val, 0xCC_BB_AA);

        Ok(())
    }

    #[test]
    fn test_skip_bytes() -> anyhow::Result<()> {
        let mut br = create_reader(&[0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);

        let _: u8 = br.read_bytes(1)?;

        br.skip_bytes(4)?;

        let val: u8 = br.read_bytes(1)?;
        assert_eq!(val, 0x06);

        Ok(())
    }

    #[test]
    fn test_eof_handling() {
        let mut br = create_reader(&[0x01]);

        let result: anyhow::Result<u16> = br.read_bits(16);

        assert!(result.is_err());

        if let Err(err_msg) = result {
            assert!(err_msg.to_string().contains("EOF"));
        }
    }
}
