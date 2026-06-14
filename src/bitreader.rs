use std::{
    fmt::Debug,
    io::{self, BufRead},
};

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

    /// Reads all bytes into buf until the delimiter byte or EOF is reached.
    ///
    /// This function is a wrapper around the underlying streams `read_until`.
    ///
    /// # Errors
    ///
    /// If the underlying stream errors.
    pub fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> io::Result<usize> {
        self.data.read_until(byte, buf)
    }

    /// Skips all bytes until the delimiter byte or EOF is reached.
    ///
    /// This function is a wrapper around the underlying streams `skip_until`.
    ///
    /// # Errors
    ///
    /// If the underlying stream errors.
    pub fn skip_until(&mut self, byte: u8) -> io::Result<usize> {
        self.data.skip_until(byte)
    }

    /// Peek at most 64 bits at a time without advancing the underlying stream.
    ///
    /// Note that the underlying stream might need to be advanced if there
    /// aren't enough bits stored in the [`BitReader`].
    ///
    /// # Errors
    ///
    /// If filling the inner buffer fails, like because of hitting EOF.
    #[inline(always)]
    pub fn peek_bits(&mut self, num_of_bits: u8) -> u128 {
        // We must advance the stream to be able to peek
        if self.num_of_stored_bits < num_of_bits {
            self.fill_inner_buffer();
        }

        self.bit_store & (1 << num_of_bits) - 1
    }

    /// Advances the underlying stream by `num_of_bits` without checks.
    ///
    /// This can result in data loss if there are less than `num_of_bits`
    /// bits stored in the internal buffer.
    #[inline(always)]
    pub const fn advance_bits_unchecked(&mut self, num_of_bits: u8) {
        self.bit_store >>= num_of_bits;
        self.num_of_stored_bits -= num_of_bits;
    }

    /// Read at most 64 bits at a time from the underlying stream.
    ///
    /// # Errors
    ///
    /// If `num_of_bits` is greater than 64, or if filling the underlying
    /// stream fails, like because of hitting EOF.
    #[inline(always)]
    pub fn read_bits(&mut self, num_of_bits: u8) -> u128 {
        let result = self.peek_bits(num_of_bits);
        self.advance_bits_unchecked(num_of_bits);
        result
    }

    #[inline]
    fn fill_inner_buffer(&mut self) {
        let buf = self.data.fill_buf().expect("Failed to fill buffer");

        let space_for_bits: usize = (128u8 - self.num_of_stored_bits).into();

        let bytes_to_process = buf.len().min(space_for_bits / 8);

        let mut scratch = [0u8; 16];

        scratch[..bytes_to_process].copy_from_slice(&buf[..bytes_to_process]);

        let bits = u128::from_le_bytes(scratch);

        self.bit_store |= bits << self.num_of_stored_bits;
        self.num_of_stored_bits += bytes_to_process as u8 * 8;

        self.data.consume(bytes_to_process);
    }

    /// Read at most 8 bytes at a time.
    ///
    /// This is just a wrapper for [`Self::read_bits`], but measured in bytes.
    ///
    /// # Errors
    ///
    /// See [`Self::read_bits`].
    #[inline(always)]
    pub fn read_bytes(&mut self, num_of_bytes: u8) -> u128 {
        self.read_bits(num_of_bytes * 8)
    }

    /// Discards any remaining bits in the current byte to align with the next
    /// byte boundary.
    #[inline]
    pub const fn align_to_byte(&mut self) {
        let leftover_bits = self.num_of_stored_bits % 8;
        if leftover_bits > 0 {
            self.bit_store >>= leftover_bits;
            self.num_of_stored_bits -= leftover_bits;
        }
    }

    /// Skip any number of bytes. The skipped bytes will be discarded.
    ///
    /// # Errors
    ///
    /// See [`Self::read_bits`].
    #[inline]
    pub fn skip_bytes(&mut self, num_of_bytes: u64) {
        let mut discard_bits = num_of_bytes * 8;
        loop {
            if discard_bits < 65 {
                let _x = self.read_bits(discard_bits.try_into().expect("32bit system moment"));
                return;
            }
            let _x = self.read_bits(64);
            discard_bits -= 64;
        }
    }

    /// Reads raw bytes exactly as they appear in the stream, preserving order.
    ///
    /// Used for magic numbers, strings, and uncompressed block payloads.
    ///
    /// # Errors
    ///
    /// See [`Self::read_bits`].
    pub fn read_raw_bytes(&mut self, buf: &mut [u8]) {
        for byte in buf.iter_mut() {
            if self.num_of_stored_bits >= 8 {
                *byte = (self.bit_store & 0xFF)
                    .try_into()
                    .expect("We masked for the bottom 8 bits");

                self.bit_store >>= 8;
                self.num_of_stored_bits -= 8;
            } else {
                let mut temp = [0u8; 1];
                self.data
                    .read_exact(&mut temp)
                    .expect("Hit EOF while reading raw bytes");
                *byte = temp[0];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Cursor;

    fn create_reader(bytes: &[u8]) -> BitReader<Cursor<Vec<u8>>> {
        BitReader::new(Cursor::new(bytes.to_vec()))
    }

    #[test]
    fn test_read_basic_bits() {
        let mut br = create_reader(&[0b1100_1010]);

        let bits1: u8 = br.read_bits(4) as u8;
        assert_eq!(bits1, 0b1010);

        let bits2: u8 = br.read_bits(4) as u8;
        assert_eq!(bits2, 0b1100);
    }

    #[test]
    fn test_peek_basic_bits() {
        let mut br = create_reader(&[0b1100_1010]);

        let bits1: u8 = br.peek_bits(4) as u8;
        assert_eq!(bits1, 0b1010);

        let bits2: u8 = br.read_bits(4) as u8;
        assert_eq!(bits2, 0b1010);

        let bits3: u8 = br.peek_bits(4) as u8;
        assert_eq!(bits3, 0b1100);

        let bits4: u8 = br.read_bits(4) as u8;
        assert_eq!(bits4, 0b1100);
    }

    #[test]
    fn test_advance_basic_bits() {
        let mut br = create_reader(&[0b1100_1010]);

        let bits1: u8 = br.peek_bits(4) as u8;
        assert_eq!(bits1, 0b1010);

        br.advance_bits_unchecked(4);

        let bits2: u8 = br.peek_bits(4) as u8;
        assert_eq!(bits2, 0b1100);
    }

    #[test]
    fn test_cross_byte_boundary() {
        // [0x33, 0x55] -> [0011_0011, 0101_0101]
        let mut br = create_reader(&[0x33, 0x55]);

        // Combined LSB first: 0101_0011_0011 -> 0x533
        let bits: u16 = br.read_bits(12) as u16;
        assert_eq!(bits, 0x533);

        let remaining: u8 = br.read_bits(4) as u8;
        assert_eq!(remaining, 0b0101);
    }

    #[test]
    fn test_align_to_byte() {
        // [0xFF, 0xAA] -> [1111_1111, 1010_1010]
        let mut br = create_reader(&[0xFF, 0xAA]);

        let _: u8 = br.read_bits(3) as u8;
        br.align_to_byte();

        let next_byte: u8 = br.read_bits(8) as u8;
        assert_eq!(next_byte, 0xAA);
    }

    #[test]
    fn test_read_dynamic_bytes() {
        let mut br = create_reader(&[0xAA, 0xBB, 0xCC, 0xDD]);

        let val: u32 = br.read_bytes(3) as u32;
        assert_eq!(val, 0xCC_BB_AA);
    }

    #[test]
    fn test_skip_bytes() {
        let mut br = create_reader(&[0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);

        let _: u8 = br.read_bytes(1) as u8;

        br.skip_bytes(4);

        let val: u8 = br.read_bytes(1) as u8;
        assert_eq!(val, 0x06);
    }
}
