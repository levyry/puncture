//! This module contains a 64 KB split linear buffer for LZ77 sliding window
//! and buffered writes.
//!
//! Traditionally, the LZ77 sliding window is implemented with a ring buffer.
//! However, a standard ring buffer is quite slow, because it doesn't allow
//! for buffered writes.
//!
//! The buffer works as follows: the bottom half of the 64 KB split buffer is
//! the LZ77 history. The top half acts as a linear buffer. Whenever the top
//! half is close to being full, the latest 32 KB of data is written to the
//! output stream, hashed with the CRC-32 hasher, and copied over to the bottom
//! half of the linear buffer. Then, the write index is reset to the halfway
//! point of the buffer, and it can continue writing data.
//!
//! This is much faster than hashing and writing the data symbol-by-symbol.

use std::io::{self, Write};

use crc32fast::Hasher;

// This is the maximum amount of data that can be written at once to the stream
const MAX_LENGTH: usize = 258;

/// The size of the LZ77 sliding window.
pub const HISTORY_SIZE: usize = 32768;
const TOTAL_SIZE: usize = HISTORY_SIZE * 2;

/// A writer that keeps a sliding window of history and calculates CRC-32 hashes
pub struct CachedWriter<W> {
    /// The wrapped stream where the writes will end up going
    pub main_stream: W,
    /// The split linear buffer
    pub buf: Box<[u8; TOTAL_SIZE]>,
    /// The write index, dictating where to write in the buffer
    pub write_index: usize,
    /// The hasher responsible for fast CRC-32 hash calculations
    pub crc32_hasher: Hasher,
}

impl<W: Write> CachedWriter<W> {
    /// Create a new [`CachedWriter`] by wrapping another stream.
    pub fn new(stream: W) -> Self {
        Self {
            main_stream: stream,
            buf: Box::new([0u8; TOTAL_SIZE]),
            write_index: HISTORY_SIZE,
            crc32_hasher: Hasher::new(),
        }
    }

    /// Write a literal byte to the stream.
    #[inline(always)]
    pub fn write_literal(&mut self, literal: u8) {
        self.buf[self.write_index] = literal;
        self.write_index += 1;
    }

    /// Repeat a specific subslice of the output stream.
    ///
    /// This function uses an exponential doubling algorithm for cases
    /// where length > distance. This means, that instead of copying
    /// `distance` amount if bits until we reach length, we copy `2 * distance`,
    /// `4 * distance`, `8 * distance`, etc. until we reach `length` amount of
    /// bits.
    #[inline(always)]
    pub fn repeat_from(&mut self, distance: usize, length: usize) {
        let start = self.write_index - distance;

        if distance >= length {
            self.buf
                .copy_within(start..start + length, self.write_index);
        } else {
            self.buf
                .copy_within(start..start + distance, self.write_index);
            let mut copied = distance;

            while copied < length {
                let to_copy = copied.min(length - copied);
                self.buf.copy_within(
                    self.write_index..self.write_index + to_copy,
                    self.write_index + copied,
                );
                copied += to_copy;
            }
        }

        self.write_index += length;
    }

    /// Flush the stream and get the final CRC-32 hash
    ///
    /// # Errors
    ///
    /// If EOF is reached at an unexpected time.
    #[inline(always)]
    pub fn finalize(mut self) -> io::Result<u32> {
        self.update_state()?;
        self.main_stream.flush()?;

        Ok(self.crc32_hasher.finalize())
    }

    /// Write and hash the contents of the writing buffer
    ///
    /// # Errors
    ///
    /// If EOF is reached at an unexpected time.
    #[inline(always)]
    fn update_state(&mut self) -> io::Result<()> {
        let written = &self.buf[HISTORY_SIZE..self.write_index];
        self.main_stream.write_all(written)?;
        self.crc32_hasher.update(written);

        Ok(())
    }

    /// Check to see if we need to empty the writing buffer
    ///
    /// # Errors
    ///
    /// If EOF is reached at an unexpected time.
    #[inline(always)]
    pub fn check_flush(&mut self) -> io::Result<()> {
        if self.write_index > TOTAL_SIZE - MAX_LENGTH {
            self.shift_to_history()?;
        }

        Ok(())
    }

    /// Shift the contents of the writing buffer over to the history half
    ///
    /// This hashes and writes the contents first, and resets the
    /// [`Self::write_index`] to [`HISTORY_SIZE`].
    ///
    /// # Errors
    ///
    /// If EOF is reached at an unexpected time.
    #[inline(always)]
    fn shift_to_history(&mut self) -> io::Result<()> {
        self.update_state()?;

        self.buf
            .copy_within(self.write_index - HISTORY_SIZE..self.write_index, 0);

        self.write_index = HISTORY_SIZE;

        Ok(())
    }
}

impl<W: Write> Write for CachedWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Only write however much we are able
        let write_len = buf.len().min(TOTAL_SIZE - self.write_index);

        self.buf[self.write_index..self.write_index + write_len].copy_from_slice(&buf[..write_len]);

        self.write_index += write_len;

        self.check_flush()?;

        Ok(write_len)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.main_stream.flush()
    }
}

#[cfg(test)]
mod tests {
    #![expect(clippy::panic_in_result_fn)]
    #![expect(clippy::indexing_slicing)]

    use super::*;
    use std::io::Write;

    // ---------------------------------------------------------
    // `write` branch coverage
    // ---------------------------------------------------------

    #[test]
    fn test_write_no_wrap() -> io::Result<()> {
        let mut out = Vec::new();
        let mut writer = CachedWriter::new(&mut out);

        writer.write_all(b"hello")?;

        assert_eq!(&writer.buf[HISTORY_SIZE..HISTORY_SIZE + 5], b"hello");
        assert_eq!(writer.write_index, HISTORY_SIZE + 5);
        Ok(())
    }

    #[test]
    fn test_write_wrap_around() -> io::Result<()> {
        let mut out = Vec::new();
        let mut writer = CachedWriter::new(&mut out);

        let padding = vec![0x01; HISTORY_SIZE - 2];
        writer.write_all(&padding)?;

        writer.write_all(b"12345")?;

        let w_idx = writer.write_index;

        assert_eq!(w_idx, HISTORY_SIZE + 5);
        assert_eq!(&writer.buf[w_idx - 5..w_idx], b"12345");
        Ok(())
    }

    #[test]
    fn test_write_exact_window_size() -> io::Result<()> {
        let mut out = Vec::new();
        let mut writer = CachedWriter::new(&mut out);

        let data = vec![0x55; HISTORY_SIZE];
        writer.write_all(&data)?;

        assert_eq!(writer.write_index, HISTORY_SIZE);
        assert_eq!(writer.buf[HISTORY_SIZE], 0x55);
        assert_eq!(writer.buf[TOTAL_SIZE - 1], 0x55);
        Ok(())
    }

    #[test]
    fn test_lz77_run_length_encoding() -> io::Result<()> {
        let mut out = Vec::new();
        let mut writer = CachedWriter::new(&mut out);

        writer.write_all(b"A")?;

        writer.repeat_from(1, 10);

        assert_eq!(
            &writer.buf[HISTORY_SIZE..writer.write_index],
            b"AAAAAAAAAAA"
        );
        assert_eq!(writer.write_index, HISTORY_SIZE + 11);
        Ok(())
    }

    #[test]
    fn test_lz77_pattern_repetition() -> io::Result<()> {
        let mut out = Vec::new();
        let mut writer = CachedWriter::new(&mut out);

        writer.write_all(b"abc")?;

        writer.repeat_from(3, 9);

        assert_eq!(
            &writer.buf[HISTORY_SIZE..writer.write_index],
            b"abcabcabcabc"
        );
        assert_eq!(writer.write_index, HISTORY_SIZE + 12);
        Ok(())
    }

    #[test]
    fn test_continuous_write_and_repeat_fixed() -> io::Result<()> {
        let mut out = Vec::new();
        let mut writer = CachedWriter::new(&mut out);

        writer.write_all(b"123")?;

        writer.repeat_from(2, 4);
        assert_eq!(&writer.buf[HISTORY_SIZE..writer.write_index], b"1232323");

        writer.write_all(b"45")?;
        assert_eq!(&writer.buf[HISTORY_SIZE..writer.write_index], b"123232345");

        writer.repeat_from(6, 3);
        assert_eq!(
            &writer.buf[HISTORY_SIZE..writer.write_index],
            b"123232345232"
        );
        Ok(())
    }
}
