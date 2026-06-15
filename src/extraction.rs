//! This module houses the DEFLATE implementation.
//!
//! ## Constants
//!
//! RFC 1951 (henceforth: "the RFC") defines a lot of constants, tables, and
//! magic numbers. These are all defined at the beginning of the module.
//!
//! ## GZIP headers
//!
//! RFC 1952 defines the GZIP header format. I chose to ignore a lot of fields,
//! the only optional field I parse is the original file name.
//!
//! ## DEFLATE
//!
//! The DEFLATE algorithm is a bit-based compression/decompression algorithm.
//! It uses two main ideas for compression: Huffman coding, and the LZ77
//! algorithm.
//!
//! ### Huffman coding
//!
//! Huffman coding helps with assigning shorter bit-sequences to more
//! frequently occuring symbols. For example, in English, the letter 'e' is the
//! most frequently used letter on average, so it should be coded with a
//! shorter bit-sequence than, say, 'q' or 'v'.
//!
//! Huffman coding is a form of prefix coding, which is pretty similar to how
//! phone numbers work. For more insight, I recommend
//! [An Explanation of the Deflate Algorithm](https://zlib.net/feldspar.html)
//! by Antaeus Feldspar. The gist of it is that the symbols are arrenged in a
//! binary tree, where only the leaf nodes contain the symbols. To get the code
//! for any symbol, you travel from the root of the tree, and every time you go
//! left, you add a `0` to the code, and every time you go right, you add a `1`.
//!
//! Given a list of symbols (like words, sentences, whatever we want to assign
//! our codes to) and their frequencies, we can build a Huffman tree from which
//! each symbols Huffman code can be deduced. Then, we simply replace the
//! symbols in our original text with their respective codes, and we now have a
//! shorter message.
//!
//! To decode it, you can simply start reading the bit-stream, and eagerly match
//! on any Huffman codes you find. Since only the leaf nodes of the tree
//! contain symbols, you will never be in an ambigous situtation.
//!
//! There are additional caveats to the specific form of Huffman coding used by
//! the RFC, but it's mostly just minor details.
//!
//! ### LZ77 algorithm
//!
//! This algoritm takes a different approach: it only focuses on eliminating as
//! many repetitions as it can. When scanning the text, it keeps a history of
//! the previously parsed text, and every repetition, it replaces with a sort of
//! pointer, indicating how far back to look in the history, and how many
//! symbols to repeat.
//!
//! For example, lets say we wanted to decompress the following input:
//!
//! ```text
//! criss [D=6,L=2]o[D=6,L=2] apple sauce
//! ```
//!
//! When we see a `[D,L]` pair, it indicates the Distance we must go backward
//! in the stream and the Length of the data we must copy. The above example
//! decodes to:
//!
//! ```text
//! criss cross apple sauce
//! ```
//!
//! The `cr` and `ss` parts are repeated between `criss` and `cross`. Of
//! course, this compression only makes sense if we copy a large amount of data,
//! not just two characters.
//!
//! ### DEFLATE, in detail
//!
//! DEFLATE works by combining these two algorithms: first it eliminates
//! duplicates with LZ77, then uses Huffman coding to shrink the input down even
//! further. There are three kinds of compression it can choose from:
//!
//! * No compression at all (mainly used for wrapping already compressed data in a GZIP header format)
//! * Fixed Huffman: First compress with LZ77, and then use a predetermined Huffman coding. The RFC defines this tree, see [`FIXED_LITERALS_LUT`] and [`FIXED_DISTANCES_LUT`] to see the construction of these trees as LUTs.
//! * Dynamic Huffman: First compress with LZ77, then the compressor can analyze the data, and create a specific Huffman tree for it on the fly. Since this tree is tailor-made for the specific data, the tree itself needs to be transmitted alongside the data.
//!
//! Most GZIP files use the dynamic huffman encoding. The trickiest part of the
//! whole algorithm is being able to decode the dynamic huffman trees on the
//! fly. For more information, read the RFC.

use std::{
    cmp::Ordering,
    ffi::CString,
    hint::cold_path,
    io::{self, BufRead, Write},
};

use crate::{bitreader::BitReader, cached_writer::CachedWriter};

const GZIP_MAGIC: [u8; 2] = [0x1F, 0x8B];
const CM_DEFLATE: u8 = 8;
const MAX_CODE_LENGTH: usize = 15;

const LENGTH_BASE_TABLE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];

const LENGTH_OFFSET_BITS_TABLE: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];

const DISTANCE_BASE_TABLE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];

const DISTANCE_OFFSET_BITS_TABLE: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

/// A memory-packed Huffman symbol decoding lookup table
///
/// On the first 9 bits, the actual decoded symbol is stored. On the following
/// 4 bits, the length of the decoded symbol is stored.
///
/// The RFC defines this table in Section 3.2.6.
pub const FIXED_LITERALS_LUT: [u16; 512] = {
    let mut table = [0u16; 512];

    let mut raw_bits: usize = 0;

    let mut reversed_bits: u16;

    while raw_bits < 512 {
        reversed_bits = (raw_bits as u16).reverse_bits() >> 7;

        match reversed_bits {
            0b0_00_00_00_00..=0b0_01_01_11_11 => {
                table[raw_bits] = (7 << 9) | (256 + (reversed_bits >> 2));
            }
            0b00_11_00_00_0..=0b10_11_11_11_1 => {
                table[raw_bits] = (8 << 9) | ((reversed_bits >> 1) - 0b00_11_00_00);
            }
            0b11_00_00_00_0..=0b11_00_01_11_1 => {
                table[raw_bits] = (8 << 9) | (280 + (reversed_bits >> 1) - 0b11_00_00_00);
            }
            0b1_10_01_00_00..=0b1_11_11_11_11 => {
                table[raw_bits] = (9 << 9) | (144 + reversed_bits - 0b1_10_01_00_00);
            }
            _ => (),
        }

        raw_bits += 1;
    }

    table
};

/// A memory-packed Huffman distance decoding lookup table
///
/// On the first 9 bits, the actual decoded distance is stored. On the following
/// 3 bits, the length of the decoded symbol is stored. For the fixed Huffman
/// tree, the length of the distance is always 5 bits. I decided to redundantly
/// store it so it is compatible with the dynamic Huffman LUTs.
///
/// The RFC defines this table in Section 3.2.6.
pub const FIXED_DISTANCES_LUT: [u16; 32] = {
    let mut table = [0u16; 32];

    let mut raw_bits: usize = 0;

    let mut distance: u16;

    while raw_bits < 32 {
        distance = (raw_bits as u16).reverse_bits() >> 11;

        table[raw_bits] = (5 << 9) | distance;

        raw_bits += 1;
    }

    table
};

/// The main struct driving the extraction process
#[derive(Debug)]
pub struct Extractor<'a, R> {
    /// The input data, wrapped in a [`BitReader`].
    pub data: &'a mut BitReader<R>,
    /// The original file name in the GZIP header, if it is present.
    pub file_name: Option<CString>,
}

impl<'a, R: BufRead> Extractor<'a, R> {
    /// Creates a new [`Extractor`] by wrapping a [`BitReader`].
    pub const fn new(data: &'a mut BitReader<R>) -> Self {
        Self {
            data,
            file_name: None,
        }
    }

    /// Provides a reference to the parsed file name if it exists.
    #[must_use]
    pub const fn get_file_name(&self) -> Option<&CString> {
        self.file_name.as_ref()
    }

    /// Processes the GZIP header.
    ///
    /// It only uses a pretty minimal set of flags from the header, but is fully
    /// RFC compliant. It reads the original file name if provided.
    ///
    /// # Panics
    ///
    /// If the GZIP header isn't RFC compliant, namely:
    ///
    /// * If the magic bytes aren't correct
    /// * If the compression method isn't `0x08`
    /// * If the reserved flag bits aren't zeroed out
    pub fn process_header(&mut self) {
        let mut magic = [0; 2];

        // The magic bytes are not LSB first
        self.data.read_raw_bytes(&mut magic);

        if magic != GZIP_MAGIC {
            unreachable!("Incorrect magic: {}{}", magic[0], magic[1]);
        }

        let cm: u8 = self.data.read_bytes(1) as u8;
        if cm != CM_DEFLATE {
            unreachable!("Incorrect compression method: {cm}");
        }

        let flags: u8 = self.data.read_bytes(1) as u8;

        let fhcrc = (flags & 0x02) != 0;
        let fextra = (flags & 0x04) != 0;
        let fname = (flags & 0x08) != 0;
        let fcomment = (flags & 0x10) != 0;

        if flags & 0xE0 != 0 {
            unreachable!("Flag reserved bits aren't zeroed out: {flags}");
        }

        // We skip MTIME, XFL and OS headers.
        let _mtime: u32 = self.data.read_bytes(4) as u32;
        let _xfl_and_os: u16 = self.data.read_bytes(2) as u16;

        if fextra {
            let xlen: u16 = self.data.read_bytes(2) as u16;
            self.data.skip_bytes(xlen.into());
        }

        if fname {
            let mut name = Vec::new();
            loop {
                let mut byte = [0u8; 1];
                self.data.read_raw_bytes(&mut byte);
                name.push(byte[0]);
                if byte[0] == 0 {
                    break;
                }
            }
            self.file_name = Some(
                CString::from_vec_with_nul(name).expect("A null byte is always the last element."),
            );
        }

        if fcomment {
            loop {
                let mut byte = [0u8; 1];
                self.data.read_raw_bytes(&mut byte);
                if byte[0] == 0 {
                    break;
                }
            }
        }
        // TODO: Currently, the crc16 field is ignored if it exists.
        // I could calculate this, but then I would need to keep a
        // seperate buffer for all the header fields I read in.
        let mut _crc16: Option<u16> = fhcrc.then(|| self.data.read_bytes(2) as u16);
    }

    /// Runs the DEFLATE algorithm and writes the result to output.
    ///
    /// It closely follows the algorithm outlined in Section 3.2.3. of the RFC.
    ///
    /// # Errors
    ///
    /// If EOF is reached at an unexpected moment.
    ///
    /// # Panics
    ///
    /// If the DEFLATE block isn't RFC compliant, namely:
    ///
    /// * If the btype header is `0b11`
    ///
    /// or if the CRC-32 checksum isn't correct.
    pub fn deflate(mut self, output: &mut impl Write) -> io::Result<()> {
        // To track the LZ77 sliding window and CRC-32 hash, we wrap the stream
        let mut output = CachedWriter::new(output);

        loop {
            let bfinal: u8 = self.data.read_bits(1) as u8;
            let btype: u8 = self.data.read_bits(2) as u8;

            match btype {
                // No compression
                0b00 => self.uncompressed_data(&mut output)?,
                // Fixed huffman
                0b01 => self.decode_huffman(
                    9,
                    5,
                    &FIXED_LITERALS_LUT,
                    &FIXED_DISTANCES_LUT,
                    &mut output,
                )?,
                // Dynamic huffman
                0b10 => {
                    let (literals, distances) = self.decode_dynamic_tables();

                    self.decode_huffman(15, 15, &literals, &distances, &mut output)?;
                }
                0b11 => unreachable!("Hit reserved Huffman btype header: 11"),
                _ => unreachable!("We only read two bits"),
            }

            if bfinal != 0 {
                break;
            }
        }

        // We might not be at a byte boundary after finishing the block
        self.data.align_to_byte();

        let expected_crc32: u32 = self.data.read_bytes(4) as u32;
        // let expected_isize: u32 = self.data.read_bytes(4) as u32;

        let calculated_crc = output.finalize()?;

        if calculated_crc != expected_crc32 {
            unreachable!(
                "Calculated crc32 hash ({calculated_crc}) doesn't match expected ({expected_crc32})."
            );
        }

        // if calculated_isize != expected_isize {
        //     unreachable!(
        //         "Actual payload size ({calculated_isize}) doesn't match expected ({expected_isize})."
        //     )
        // }

        Ok(())
    }

    /// Decompresses an uncompressed DEFLATE block
    ///
    /// # Errors
    ///
    /// If an EOF is reached unexpectedly.
    ///
    /// # Panics
    ///
    /// If the DEFLATE block isn't RFC compliant, namely:
    ///
    /// * If `len` and `nlen` aren't one's complements of each other.
    pub fn uncompressed_data<W: Write>(&mut self, output: &mut CachedWriter<W>) -> io::Result<()> {
        self.data.align_to_byte();
        let len: u16 = self.data.read_bytes(2) as u16;
        let nlen: u16 = self.data.read_bytes(2) as u16;

        if len != !nlen {
            unreachable!("Member nlen isn't one's complement of len.",);
        }

        let mut payload = vec![0u8; len.into()];

        self.data.read_raw_bytes(&mut payload);

        output.write_all(&payload)?;

        Ok(())
    }

    /// Decodes a Huffman encoded DEFLATE block
    ///
    /// This function is used for both fixed and dynamic Huffman decompression.
    ///
    /// For further information, read the inline comments and Section 3.2.3.
    /// of the RFC.
    ///
    /// # Errors
    ///
    /// If EOF is reached unexpectedly.
    #[inline(always)]
    pub fn decode_huffman<W: Write>(
        &mut self,
        literal_max_length: u8,
        distance_max_length: u8,
        literals: &[u16],
        distances: &[u16],
        output: &mut CachedWriter<W>,
    ) -> io::Result<()> {
        loop {
            // Check if we have enough space in the buffer for another literal.
            output.check_flush()?;

            // First, decode the literal, to see whether this is a Distance/
            // Length pair, or just a regular Huffman code
            let literal_bits: u16 = self.data.peek_bits(literal_max_length) as u16;
            let symbol_mask = (1u16 << literal_max_length) - 1;
            let packed_symbol = literals[usize::from(literal_bits & symbol_mask)];
            let literal = packed_symbol & 0x1FF;
            let literal_len = (packed_symbol >> 9) as u8;
            self.data.advance_bits_unchecked(literal_len);

            // As per Section 3.2.3.
            match literal.cmp(&256) {
                Ordering::Less => output.write_literal(literal as u8),
                Ordering::Equal => {
                    // 256 denotes the end of this block
                    cold_path();
                    break;
                }
                Ordering::Greater => {
                    // We have a length/distance pair. The length is already encoded
                    // in the literal
                    let length_index: usize = (literal - 257).into();

                    let length_base = LENGTH_BASE_TABLE[length_index];
                    let length_offset_bits = LENGTH_OFFSET_BITS_TABLE[length_index];

                    let length_offset: u16 = self.data.read_bits(length_offset_bits) as u16;

                    let length: usize = (length_base + length_offset).into();

                    // The distance is right after the length in the stream
                    let distance_bits: u16 = self.data.peek_bits(distance_max_length) as u16;

                    let distance_mask = (1u16 << distance_max_length) - 1;
                    let packed_distance = distances[usize::from(distance_bits & distance_mask)];

                    let distance_index = usize::from(packed_distance & 0x1FF);
                    let distance_len = (packed_distance >> 9) as u8;
                    self.data.advance_bits_unchecked(distance_len);

                    let distance_base = DISTANCE_BASE_TABLE[distance_index];
                    let distance_offset_bits = DISTANCE_OFFSET_BITS_TABLE[distance_index];

                    let distance_offset: u16 = self.data.read_bits(distance_offset_bits) as u16;

                    let distance = (distance_base + distance_offset).into();

                    // Check the LZ77 sliding window, and repeat `length`
                    // bits from `distance`.
                    output.repeat_from(distance, length);
                }
            }
        }

        Ok(())
    }

    /// Decode the dynamic Huffman tables
    ///
    /// The main algorithm and format is described in Section 3.2.2. and 3.2.7.
    /// respectively.
    ///
    /// It first decodes the "code length table", and using that it builds
    /// the actual length and distance tables.
    pub fn decode_dynamic_tables(&mut self) -> ([u16; 32768], [u16; 32768]) {
        let hlit = self.data.read_bits(5) as u16 + 257;
        let hdist = self.data.read_bits(5) as u16 + 1;
        let hclen = self.data.read_bits(4) as u8 + 4;

        let mut code_lengths_scratch: u64 = self.data.read_bits(hclen * 3) as u64;

        let mut codelength_lengths = [0u16; 19];

        // RFC defined sequence
        for index in [
            16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
        ] {
            let value = code_lengths_scratch & 0x7;
            codelength_lengths[index] = value as u16;
            code_lengths_scratch >>= 3;
        }

        let mut codelength_codes = [0u16; 19];

        build_huff_codes(&codelength_lengths, &mut codelength_codes);

        let codelength_lut = build_lut::<128>(&codelength_lengths, &codelength_codes);

        let mut lit_dist_table = [0u16; 286 + 32];

        // Now we need to read the next `hlit + hdist` symbols and decode them
        let mut index = 0;
        let symbol_count = (hlit + hdist).into();
        while index != symbol_count {
            let bits: u8 = self.data.peek_bits(7) as u8;

            let packed_value = codelength_lut[(bits & 0x7F) as usize];
            let symbol: u16 = packed_value & 0x1FF;

            let symbol_len = (packed_value >> 9) as u8;
            self.data.advance_bits_unchecked(symbol_len);

            // As per the code length alphabet described in Section 3.2.7.
            match symbol {
                0..=15 => {
                    lit_dist_table[index] = symbol;
                    index += 1;
                }
                16 => {
                    let prev = lit_dist_table[index - 1];

                    let repeat_length = (self.data.read_bits(2) + 3) as usize;

                    for repeat_index in 0..repeat_length {
                        lit_dist_table[index + repeat_index] = prev;
                    }

                    index += repeat_length;
                }
                17 => {
                    let repeat_length = (self.data.read_bits(3) + 3) as usize;

                    for repeat_index in 0..repeat_length {
                        lit_dist_table[index + repeat_index] = 0;
                    }

                    index += repeat_length;
                }
                18 => {
                    let repeat_length = (self.data.read_bits(7) + 11) as usize;

                    for repeat_index in 0..repeat_length {
                        lit_dist_table[index + repeat_index] = 0;
                    }

                    index += repeat_length;
                }
                _ => unreachable!("Wrong symbol while building lit/dist table: {symbol}"),
            }
        }

        // We have the codelengths for the literals/lengths, and the distances
        let (lit_lengths, dist_lengths) = lit_dist_table.split_at(hlit.into());

        // Using these, we can build the tables.
        let mut lit_codes = vec![0u16; hlit.into()];
        build_huff_codes(lit_lengths, &mut lit_codes);
        let lit_table = build_lut::<32768>(lit_lengths, &lit_codes);

        let mut dist_codes = vec![0u16; hdist.into()];
        build_huff_codes(dist_lengths, &mut dist_codes);
        let dist_table = build_lut::<32768>(dist_lengths, &dist_codes);

        (lit_table, dist_table)
    }
}

/// Creates the Huffman codes from just the list of codelengths
///
/// The Huffman codes list is indexed by the symbol, and it returns the
/// associated code.
///
/// This implementation follows the steps listed in Section 3.2.2. of the RFC.
pub fn build_huff_codes(lengths: &[u16], codes: &mut [u16]) {
    let bl_count: [u16; MAX_CODE_LENGTH + 1] = {
        let mut counts = [0u16; MAX_CODE_LENGTH + 1];

        for &bit_length in lengths {
            counts[bit_length as usize] += 1;
        }

        counts[0] = 0;

        counts
    };

    let mut next_code: [u16; MAX_CODE_LENGTH + 1] = {
        let mut next = [0u16; MAX_CODE_LENGTH + 1];
        let mut code = 0;
        for index in 1..=MAX_CODE_LENGTH {
            code = (code + bl_count[index - 1]) << 1;
            next[index] = code;
        }

        next
    };

    for (index, &code_length) in lengths.iter().enumerate() {
        if code_length != 0 {
            let code = next_code[code_length as usize].reverse_bits() >> (16 - code_length);
            codes[index] = code;
            next_code[code_length as usize] += 1;
        }
    }
}

/// Create a memory-packed LUT for decoding Huffman codes to symbols
///
/// It takes a list of codelengths and the codes themselves. For each Huffman
/// code with length > 0, we create an entry in the table. The first 9 bits
/// contain the symbol (with potentially some extra bits), the rest contain the
/// length of the symbol.
///
/// ## Optimizations
///
/// These tables can be very large (as with the dynamic huffman tables), and
/// they don't fit in most L1 caches. A nice optimization would be a two-tier
/// LUT, which would use a smaller primary table for the more frequent symbols,
/// and a secondary table for the more rare ones. This way, the whole primary
/// LUT could stay in L1 cache. This would provide a pretty significant boost
/// in execution speed, but I chose not to implement it, since it borders on
/// the complexity I wanted to avoid.
#[must_use]
pub fn build_lut<const LUT: usize>(lengths: &[u16], codes: &[u16]) -> [u16; LUT] {
    let mut table = [0u16; LUT];

    for (symbol, &length) in lengths.iter().enumerate() {
        if length == 0 {
            continue;
        }

        let mut index = codes[symbol] as usize;

        let step = 1 << length;

        while index < LUT {
            table[index] = (length << 9) | symbol as u16;
            index += step;
        }
    }

    table
}
