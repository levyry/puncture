use std::io;

const STORE_SIZE: u8 = 128;

#[derive(Debug)]
pub struct BitWriter<W> {
    /// The wrapped output stream
    pub data: W,
    /// The internal bit buffer
    pub bit_store: u128,
    /// The amount of bits actually stored in the buffer
    pub bit_count: u8,
}

impl<W: io::Write> BitWriter<W> {
    pub const fn new(data: W) -> Self {
        Self {
            data,
            bit_store: 0,
            bit_count: 0,
        }
    }

    /// Pack bits LSB-first into the internal store
    pub fn write_bits(&mut self, bits: u16, count: u8) -> io::Result<()> {
        if self.bit_count + count >= STORE_SIZE {
            self.flush_bytes()?;
        }

        let bits = u128::from(bits);
        let mask = (1 << count) - 1;

        self.bit_store |= (bits & mask) << self.bit_count;
        self.bit_count += count;

        Ok(())
    }

    /// Aligns the bit stream to the next byte boundary by padding with zeroes,
    /// and then flushes the buffer to the underlying stream.
    pub fn force_align(&mut self) -> io::Result<()> {
        let remainder = self.bit_count % 8;
        if remainder != 0 {
            self.bit_count += 8 - remainder;
        }

        self.flush_bytes()
    }

    /// Flush any full bytes to the underlying stream
    fn flush_bytes(&mut self) -> io::Result<()> {
        let full_byte_count = self.bit_count.saturating_div(8);

        let buf = self.bit_store.to_le_bytes();

        self.data
            .write_all(&buf.as_slice()[..full_byte_count as usize])?;

        self.bit_store >>= full_byte_count.saturating_mul(8);
        self.bit_count -= full_byte_count.saturating_mul(8);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn test_write_basic_bits() {
        let mut bw = BitWriter::new(Vec::new());

        bw.write_bits(0b1010, 4).unwrap();
        assert_eq!(bw.bit_store, 0b1010);
        assert_eq!(bw.bit_count, 4);

        bw.write_bits(0b1100, 4).unwrap();
        assert_eq!(bw.bit_store, 0b1100_1010);
        assert_eq!(bw.bit_count, 8);
    }

    #[test]
    fn test_flush_exact_byte() {
        let mut bw = BitWriter::new(Vec::new());

        bw.write_bits(0b1100_1010, 8).unwrap();
        bw.flush_bytes().unwrap();

        assert_eq!(bw.data, vec![0b1100_1010]);
        assert_eq!(bw.bit_count, 0);
        assert_eq!(bw.bit_store, 0);
    }

    #[test]
    fn test_cross_byte_boundary() {
        let mut bw = BitWriter::new(Vec::new());

        bw.write_bits(0x533, 12).unwrap();
        bw.flush_bytes().unwrap();

        assert_eq!(bw.data, vec![0x33]);

        assert_eq!(bw.bit_count, 4);
        assert_eq!(bw.bit_store, 0x05);
    }

    #[test]
    fn test_force_align() {
        let mut bw = BitWriter::new(Vec::new());

        bw.write_bits(0b101, 3).unwrap();

        bw.force_align().unwrap();

        assert_eq!(bw.data, vec![0x05]);
        assert_eq!(bw.bit_count, 0);
        assert_eq!(bw.bit_store, 0);
    }
}
