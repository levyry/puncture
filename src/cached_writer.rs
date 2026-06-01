/// This module houses [`CachedWriter`], which is used for the sliding window
/// of the LZ77 algorithm.
///
/// It is implemented as a fixed-size, overwriting circular buffer. It wraps a
/// "main stream" which houses the actual data that gets written, but it also
/// manages `buf`, which is the sliding window that keeps a fixed size history
/// of what was written into the stream.
///
/// There are some LZ77 specific utility functions for making look-back easier.
use std::io::{self, Write};

use anyhow::{Result, anyhow, bail};

use crate::crc32::Crc32;

pub const WINDOW_SIZE: usize = 32768;

/// A writer that wraps another stream while also keeping a cache of previous
/// writes.
pub struct CachedWriter<W> {
    main_stream: W,
    buf: Box<[u8; WINDOW_SIZE]>,
    write_index: usize,
}

impl<W: Write> CachedWriter<W> {
    /// Create a new [`CachedWriter`] by wrapping another stream.
    pub fn new(stream: W) -> Self {
        let lookback_vec = vec![0u8; WINDOW_SIZE];

        let Ok(buf) = lookback_vec.into_boxed_slice().try_into() else {
            unreachable!();
        };

        Self {
            main_stream: stream,
            buf,
            write_index: 0,
        }
    }

    /// Repeat a specific subslice of the output stream.
    pub fn repeat_from(&mut self, distance: usize, length: usize) -> Result<()> {
        if distance == 0 {
            bail!("LZ77 distance cannot be zero")
        }

        if distance > WINDOW_SIZE {
            bail!("LZ77 distance larger than cache window");
        }

        let mut bytes_written = 0;

        let mut scratch = [0u8; 258];

        let start_offset = WINDOW_SIZE.saturating_sub(distance);

        while bytes_written != length {
            let start = self.write_index.saturating_add(start_offset) % WINDOW_SIZE;

            let end_offset = usize::min(distance, length.saturating_sub(bytes_written));

            let end = start.saturating_add(end_offset) % WINDOW_SIZE;

            if start < end
                && let Some(range_to_copy) = self.buf.get(start..end)
            {
                let amount_wrote = range_to_copy.len();
                scratch[..amount_wrote].copy_from_slice(range_to_copy);
                self.write_all(&mut scratch[..amount_wrote])?;
            } else if let Some(start_to_back) = self.buf.get(start..)
                && let Some(front_to_end) = self.buf.get(..end)
            {
                let start_to_back_len = start_to_back.len();
                let front_to_end_len = front_to_end.len();
                let amount_wrote = start_to_back_len + front_to_end_len;
                scratch[..start_to_back_len].copy_from_slice(start_to_back);
                scratch[start_to_back_len..amount_wrote].copy_from_slice(front_to_end);
                self.write_all(&mut scratch[..amount_wrote])?;
                scratch = [0u8; 258];
            }

            bytes_written = bytes_written.saturating_add(end_offset);
        }

        Ok(())
    }
}

impl<W: Write> CachedWriter<Crc32<W>> {
    pub const fn get_hashes(&self) -> (u32, u32) {
        self.main_stream.get_hashes()
    }
}

impl<W: Write> Write for CachedWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = self.main_stream.write(buf)?;

        // If we wrote more than WINDOW_SIZE, we only care about caching the
        // most recent tail.
        let cache_data = if written > WINDOW_SIZE
            && let Some(buffer) = buf.get(written.saturating_sub(WINDOW_SIZE)..written)
        {
            buffer
        } else if let Some(buffer) = buf.get(..written) {
            buffer
        } else {
            return Err(io::Error::other(anyhow!("CachedWriter got corrupted")));
        };

        let cache_len = cache_data.len();
        let buf_midpoint = WINDOW_SIZE.saturating_sub(self.write_index);

        let end = self.write_index.saturating_add(cache_len);

        if end <= WINDOW_SIZE
            && let Some(buffer) = self.buf.get_mut(self.write_index..end)
            && let Some(new_data) = cache_data.get(..cache_len)
        {
            buffer.copy_from_slice(new_data);
        } else if let Ok([buffer1, buffer2]) = self
            .buf
            .get_disjoint_mut([self.write_index..WINDOW_SIZE, 0..(end % WINDOW_SIZE)])
            && let Some(new_data1) = cache_data.get(..buf_midpoint)
            && let Some(new_data2) = cache_data.get(buf_midpoint..cache_len)
        {
            buffer1.copy_from_slice(new_data1);
            buffer2.copy_from_slice(new_data2);
        } else {
            return Err(io::Error::other(anyhow!("CachedWriter got corrupted")));
        }

        self.write_index = end % WINDOW_SIZE;

        Ok(written)
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
    fn test_write_no_wrap() -> Result<()> {
        // Branch: `if end < WINDOW_SIZE`
        let mut out = Vec::new();
        let mut writer = CachedWriter::new(&mut out);

        writer.write_all(b"hello")?;

        assert_eq!(writer.main_stream, b"hello");
        assert_eq!(writer.write_index, 5);
        assert_eq!(&writer.buf[..5], b"hello");
        Ok(())
    }

    #[test]
    fn test_write_wrap_around() -> Result<()> {
        // Branch: `else if let Ok([buffer1, buffer2]) = ...`
        let mut out = Vec::new();
        let mut writer = CachedWriter::new(&mut out);

        let padding = vec![0x01; WINDOW_SIZE - 2];
        writer.write_all(&padding)?;

        writer.write_all(b"12345")?;

        assert_eq!(writer.write_index, 3);
        assert_eq!(&writer.buf[WINDOW_SIZE - 2..WINDOW_SIZE], b"12");
        assert_eq!(&writer.buf[..3], b"345");
        Ok(())
    }

    #[test]
    fn test_write_exact_window_size() -> Result<()> {
        let mut out = Vec::new();
        let mut writer = CachedWriter::new(&mut out);

        let data = vec![0x55; WINDOW_SIZE];
        writer.write_all(&data)?;

        assert_eq!(writer.write_index, 0);
        assert_eq!(writer.buf[0], 0x55);
        assert_eq!(writer.buf[WINDOW_SIZE - 1], 0x55);
        Ok(())
    }

    #[test]
    fn test_lz77_run_length_encoding() -> Result<()> {
        let mut out = Vec::new();
        let mut writer = CachedWriter::new(&mut out);

        writer.write_all(b"A")?;

        writer.repeat_from(1, 10)?;

        assert_eq!(writer.main_stream, b"AAAAAAAAAAA");
        assert_eq!(writer.write_index, 11);
        Ok(())
    }

    #[test]
    fn test_lz77_pattern_repetition() -> Result<()> {
        let mut out = Vec::new();
        let mut writer = CachedWriter::new(&mut out);

        writer.write_all(b"abc")?;

        writer.repeat_from(3, 9)?;

        assert_eq!(writer.main_stream, b"abcabcabcabc");
        assert_eq!(writer.write_index, 12);
        Ok(())
    }

    #[test]
    fn test_continuous_write_and_repeat_fixed() -> Result<()> {
        let mut out = Vec::new();
        let mut writer = CachedWriter::new(&mut out);

        writer.write_all(b"123")?;

        writer.repeat_from(2, 4)?;
        assert_eq!(writer.main_stream, b"1232323");

        writer.write_all(b"45")?;
        assert_eq!(writer.main_stream, b"123232345");

        writer.repeat_from(6, 3)?;

        assert_eq!(writer.main_stream, b"123232345232");
        Ok(())
    }

    #[test]
    fn test_repeat_from_max_distance() -> Result<()> {
        let mut out = Vec::new();
        let mut writer = CachedWriter::new(&mut out);

        let payload = vec![0xAB; WINDOW_SIZE];
        writer.write_all(&payload)?;

        writer.repeat_from(WINDOW_SIZE, 10)?;

        assert_eq!(writer.main_stream.len(), WINDOW_SIZE + 10);
        assert_eq!(
            &writer.main_stream[WINDOW_SIZE..],
            vec![0xAB; 10].as_slice()
        );

        assert_eq!(writer.write_index, 10);

        Ok(())
    }
}
