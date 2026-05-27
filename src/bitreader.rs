use std::io::{self, BufRead};

use anyhow::{Context, Result, bail};

/// A reader that can extract bits LSB first.
#[derive(Debug)]
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

    pub fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> io::Result<usize> {
        self.data.read_until(byte, buf)
    }

    pub fn skip_until(&mut self, byte: u8) -> io::Result<usize> {
        self.data.skip_until(byte)
    }

    /// Read at most 64 bits at a time.
    pub fn read_bits<T: TryFrom<u128>>(&mut self, num_of_bits: u8) -> Result<T> {
        if num_of_bits > 64 {
            bail!("Tried reading more than 64 bits at a time: {num_of_bits}");
        }

        if num_of_bits == 0 {
            let result = 0.try_into().map_err(|_| unreachable!());

            return result;
        }

        // Read from the buffer until we have enough bits
        while self.num_of_stored_bits < num_of_bits {
            let buf = self.data.fill_buf().context("Failed to fill buffer")?;

            if buf.is_empty() {
                bail!(
                    "Hit EOF while filling buffer for requested bits. Number of bits requested: {num_of_bits}"
                );
            }

            // Calculate how many bytes we can safely fit into the remaining
            // space of our u128. Since max num_of_bits is 64, this will always
            // be at least 8, ensuring we make progress
            let space_for_bytes = ((u8::saturating_sub(128, self.num_of_stored_bits)) / 8).into();

            let bytes_to_process = buf.len().min(space_for_bytes);

            buf.iter().take(bytes_to_process).for_each(|&byte| {
                let scratch: u128 = byte.into();
                self.bit_store |= scratch << self.num_of_stored_bits;
                self.num_of_stored_bits = self.num_of_stored_bits.saturating_add(8);
            });

            // for &byte in &buf[..bytes_to_process] {
            //     let scratch: u128 = byte.into();
            //     self.bit_store |= scratch << self.num_of_stored_bits;
            //     self.num_of_stored_bits = self.num_of_stored_bits.saturating_add(8);
            // }

            // Mark read bytes as consumed
            self.data.consume(bytes_to_process);
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
    pub fn read_bytes<T: TryFrom<u128>>(&mut self, num_of_bytes: u8) -> Result<T> {
        let bits = num_of_bytes.saturating_mul(8);
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
        let mut discard_bits = num_of_bytes.saturating_mul(8);
        loop {
            if discard_bits < 65 {
                let _x: u64 = self.read_bits(discard_bits.try_into()?)?;
                return Ok(());
            }
            let _x: u64 = self.read_bits(64)?;
            discard_bits = discard_bits.saturating_sub(64);
        }
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
