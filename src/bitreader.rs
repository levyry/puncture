use std::io::Read;

pub struct BitReader<R> {
    data: R,
    bit_store: u64,
    num_of_stored_bits: u8,
}

impl<R: Read> BitReader<R> {
    pub const fn new(data: R) -> Self {
        Self {
            data,
            bit_store: 0,
            num_of_stored_bits: 0,
        }
    }

    pub fn read_bits(&mut self, num_of_bits: u8) -> std::io::Result<u32> {
        // Read until we have enough bits
        while self.num_of_stored_bits < num_of_bits {
            let mut scratch = [0; 2];

            self.data.read_exact(&mut scratch)?;

            self.bit_store |= (scratch[0] as u64) << self.num_of_stored_bits;
            self.num_of_stored_bits += 16;
        }

        // Get result
        let mask = (1 << num_of_bits) - 1;
        let result = (self.bit_store & mask) as u32;

        // Clear out internal state
        self.bit_store >>= num_of_bits;
        self.num_of_stored_bits -= num_of_bits;

        Ok(result)
    }

    pub fn read_bytes(&mut self, num_of_bytes: u8) -> std::io::Result<u32> {
        self.read_bits(num_of_bytes * 8)
    }
}

mod tests {
    use std::io;

    use super::*;

    #[test]
    fn read_bits() -> io::Result<()> {
        let input = vec![0b0011_0011, 0x34, 0x56];
        let mut br = BitReader::new(input.as_slice());

        let bits = br.read_bits(3)?;
        assert_eq!(bits, 0b00000_011);

        let bits = br.read_bits(3)?;
        assert_eq!(bits, 0b00000_110);

        Ok(())
    }
}
