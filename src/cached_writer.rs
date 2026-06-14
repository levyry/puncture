use std::io::{self, Write};

use crc32fast::Hasher;

const MAX_LENGTH: usize = 258;
pub const HISTORY_SIZE: usize = 32768;
pub const TOTAL_SIZE: usize = HISTORY_SIZE * 2;

/// A writer that wraps another stream while also keeping a cache of previous
/// writes.
pub struct CachedWriter<W> {
    main_stream: W,
    buf: Box<[u8; TOTAL_SIZE]>,
    write_index: usize,
    crc32_hasher: Hasher,
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

    #[inline(always)]
    pub fn write_literal(&mut self, literal: u8) {
        self.buf[self.write_index] = literal;
        self.write_index += 1;
    }

    /// Repeat a specific subslice of the output stream.
    #[inline(always)]
    pub fn repeat_from(&mut self, distance: usize, length: usize) {
        let start = self.write_index - distance;
        let end = start + length;

        // We might copy garbage here, but overwrite it in the while loop later
        self.buf.copy_within(start..end, self.write_index);
        self.write_index += length.min(distance);

        if length > distance {
            let mut amount_wrote = distance;

            while amount_wrote != length {
                let chunk_size = amount_wrote.min(length - amount_wrote);
                let end = start + chunk_size;

                self.buf.copy_within(start..end, self.write_index);

                self.write_index += chunk_size;
                amount_wrote += chunk_size;
            }
        }
    }

    #[inline(always)]
    pub fn finalize(mut self) -> io::Result<u32> {
        self.update_state()?;
        self.main_stream.flush()?;

        Ok(self.crc32_hasher.finalize())
    }

    #[inline(always)]
    fn update_state(&mut self) -> io::Result<()> {
        let written = &self.buf[HISTORY_SIZE..self.write_index];
        self.main_stream.write_all(written)?;
        self.crc32_hasher.update(written);

        Ok(())
    }

    #[inline(always)]
    pub fn check_flush(&mut self) -> io::Result<()> {
        if self.write_index > TOTAL_SIZE - MAX_LENGTH {
            self.shift_to_history()?;
        }

        Ok(())
    }

    #[inline(always)]
    fn shift_to_history(&mut self) -> Result<(), io::Error> {
        self.update_state()?;

        self.buf
            .copy_within(self.write_index - HISTORY_SIZE..self.write_index, 0);

        self.write_index = HISTORY_SIZE;

        Ok(())
    }
}

impl<W: Write> Write for CachedWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
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
