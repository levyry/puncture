use std::io;

use crc32fast::Hasher;

use crate::{
    bitwriter::BitWriter,
    extraction::{
        DISTANCE_BASE_TABLE, DISTANCE_OFFSET_BITS_TABLE, LENGTH_BASE_TABLE,
        LENGTH_OFFSET_BITS_TABLE,
    },
};

pub const HISTORY_SIZE: usize = 32768;
const TOTAL_SIZE: usize = HISTORY_SIZE * 2;

// A 15-bit hash means we have 32,768 possible hash values.
const HASH_SIZE: usize = 32768;
const HASH_MASK: usize = HASH_SIZE - 1;

// Minimum and maximum match lengths as defined by the DEFLATE RFC.
const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 258;

#[derive(Debug)]
pub struct Compressor<W> {
    pub bit_writer: BitWriter<W>,

    /// The uncompressed sliding window data
    pub data: Box<[u8; TOTAL_SIZE]>,

    /// The hash table entry points
    pub head: Box<[u16; HASH_SIZE]>,

    /// The linked list of previous occurrences
    pub prev: Box<[u16; HISTORY_SIZE]>,

    pub current_pos: usize,
    pub lookahead: usize,
    pub crc32_hasher: Hasher,
    pub payload_size: u32,
}

impl<W: io::Write> Compressor<W> {
    pub fn new(stream: W) -> Self {
        Self {
            bit_writer: BitWriter::new(stream),
            data: Box::new([0u8; TOTAL_SIZE]),
            head: Box::new([0u16; HASH_SIZE]),
            prev: Box::new([0u16; HISTORY_SIZE]),
            current_pos: 0,
            lookahead: 0,
            crc32_hasher: Hasher::new(),
            payload_size: 0,
        }
    }

    // TODO: don't start the block here, and add actual header logic
    pub fn write_header(&mut self) -> io::Result<()> {
        self.bit_writer
            .data
            .write_all(&[0x1F, 0x8B, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF])?;

        // Use fixed Huffman for now
        self.bit_writer.write_bits(0b011, 3)?;

        Ok(())
    }

    /// Fast 3-byte integer hash
    #[inline(always)]
    const fn hash_3_bytes(sequence: &[u8]) -> usize {
        let h = (sequence[0] as usize) << 10 ^ (sequence[1] as usize) << 5 ^ (sequence[2] as usize);
        h & HASH_MASK
    }

    fn compress_data(&mut self) -> io::Result<()> {
        while self.lookahead > MIN_MATCH {
            let (best_length, best_distance) = self.find_best_match();

            if best_length < MIN_MATCH {
                // Emit a literal
                let literal = self.data[self.current_pos];

                let (bit_length, base_code) = match literal {
                    0..=143 => (8, u16::from(literal) + 0x30),
                    144..=255 => (9, u16::from(literal) + 0x100),
                };

                self.bit_writer
                    .write_bits(base_code.reverse_bits() >> (16 - bit_length), bit_length)?;

                self.current_pos += 1;
                self.lookahead -= 1;
            } else {
                // Emit a length followed by a distance. Length first:
                let mut index = 0;
                while index < 28 && LENGTH_BASE_TABLE[index + 1] <= best_length as u16 {
                    index += 1;
                }

                let len_symbol = 257 + index as u16;

                let (len_bit_length, len_base_code) = match len_symbol {
                    257..=279 => (7, len_symbol - 256),
                    280..=285 => (8, len_symbol - 280 + 0xC0),
                    _ => {
                        return Err(io::Error::other(format!(
                            "invalid len_symbol: {len_symbol}"
                        )));
                    }
                };

                self.bit_writer.write_bits(
                    len_base_code.reverse_bits() >> (16 - len_bit_length),
                    len_bit_length,
                )?;

                let extra_bits = best_length as u16 - LENGTH_BASE_TABLE[index];
                if LENGTH_OFFSET_BITS_TABLE[index] > 0 {
                    self.bit_writer
                        .write_bits(extra_bits, LENGTH_OFFSET_BITS_TABLE[index])?;
                }

                // Now distance
                let mut index = 0;
                while index < 29 && DISTANCE_BASE_TABLE[index + 1] <= best_distance as u16 {
                    index += 1;
                }

                let dist_symbol = index as u16;

                self.bit_writer
                    .write_bits(dist_symbol.reverse_bits() >> 11, 5)?;

                let extra_bits = best_distance as u16 - DISTANCE_BASE_TABLE[index];
                if DISTANCE_OFFSET_BITS_TABLE[index] > 0 {
                    self.bit_writer
                        .write_bits(extra_bits, DISTANCE_OFFSET_BITS_TABLE[index])?;
                }

                // Advance past the first byte because it was hashed in `find_best_match`
                self.current_pos += 1;
                self.lookahead -= 1;

                // Hash and advance the remaining bytes, safely bounded
                for _ in 1..best_length {
                    if self.lookahead >= MIN_MATCH {
                        self.update_hash_chain();
                    }
                    self.current_pos += 1;
                    self.lookahead -= 1;
                }
            }
        }

        Ok(())
    }

    fn find_best_match(&mut self) -> (usize, usize) {
        let prev_match_pos = self.update_hash_chain();

        let mut best_length = 0;
        let mut best_distance = 0;

        let mut match_pos = prev_match_pos as usize;

        let mut chain_counter = 0;

        while match_pos > 0 && (self.current_pos - match_pos) <= HISTORY_SIZE && chain_counter < 256
        {
            let mut current_length = 0;
            for i in 0..MAX_MATCH.min(self.lookahead) {
                if self.data[match_pos + i] != self.data[self.current_pos + i] {
                    break;
                }
                current_length += 1;
            }

            if current_length >= best_length {
                best_length = current_length;
                best_distance = self.current_pos - match_pos;
            }

            if best_length == MAX_MATCH {
                break;
            }

            match_pos = self.prev[match_pos % HISTORY_SIZE] as usize;
            chain_counter += 1;
        }
        (best_length, best_distance)
    }

    fn update_hash_chain(&mut self) -> u16 {
        let hash = Self::hash_3_bytes(&self.data[self.current_pos..]);

        let prev_match_pos = self.head[hash];

        self.prev[self.current_pos % HISTORY_SIZE] = prev_match_pos;
        self.head[hash] = self.current_pos as u16;

        prev_match_pos
    }

    fn slide_window(&mut self) {
        self.data.copy_within(HISTORY_SIZE..TOTAL_SIZE, 0);
        self.current_pos -= HISTORY_SIZE;

        self.head
            .iter_mut()
            .for_each(|idx| *idx = idx.saturating_sub(HISTORY_SIZE as u16));

        self.prev
            .iter_mut()
            .for_each(|idx| *idx = idx.saturating_sub(HISTORY_SIZE as u16));
    }

    pub fn finish(mut self) -> io::Result<()> {
        while self.lookahead > 0 {
            let literal = self.data[self.current_pos];

            let (bit_length, base_code) = match literal {
                0..=143 => (8, u16::from(literal) + 0x30),
                144..=255 => (9, u16::from(literal) + 0x100),
            };

            self.bit_writer
                .write_bits(base_code.reverse_bits() >> (16 - bit_length), bit_length)?;

            self.current_pos += 1;
            self.lookahead -= 1;
        }

        self.bit_writer.write_bits(0, 7)?;

        self.bit_writer.force_align()?;

        let hash = self.crc32_hasher.finalize();

        self.bit_writer.data.write_all(&hash.to_le_bytes())?;
        self.bit_writer
            .data
            .write_all(&self.payload_size.to_le_bytes())?;

        self.bit_writer.force_align()?;

        Ok(())
    }
}

// TODO: Refactor
impl<W: io::Write> io::Write for Compressor<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        self.crc32_hasher.update(buf);
        self.payload_size += buf.len() as u32;

        let mut written = 0;

        while written < buf.len() {
            let available_space = TOTAL_SIZE - (self.current_pos + self.lookahead);
            let to_write = (buf.len() - written).min(available_space);

            let dest_start = self.current_pos + self.lookahead;
            self.data[dest_start..dest_start + to_write]
                .copy_from_slice(&buf[written..written + to_write]);

            self.lookahead += to_write;
            written += to_write;

            self.compress_data()?;

            if self.current_pos + self.lookahead >= TOTAL_SIZE {
                self.slide_window();
            }
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.bit_writer.force_align()
    }
}
