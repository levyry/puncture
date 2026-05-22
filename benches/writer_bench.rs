#![expect(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::missing_errors_doc
)]

use anyhow::{Result, bail};
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use std::io::{self, Write, sink};

const WINDOW_SIZE: usize = 32768;

// ==========================================
// Current impl
// ==========================================
pub struct CurrentCachedWriter<W> {
    main_stream: W,
    buf: Box<[u8; WINDOW_SIZE]>,
    write_index: usize,
}

impl<W: Write> CurrentCachedWriter<W> {
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

    pub fn repeat_from(&mut self, distance: usize, length: usize) -> Result<()> {
        if distance > WINDOW_SIZE {
            bail!("Trying to read too far back in LZ77 decoding");
        }

        let mut bytes_written = 0;

        let start_offset = WINDOW_SIZE.saturating_sub(distance);

        while bytes_written != length {
            let start = self.write_index.saturating_add(start_offset) % WINDOW_SIZE;

            let end_offset = usize::min(distance, length.saturating_sub(bytes_written));

            let end = start.saturating_add(end_offset) % WINDOW_SIZE;

            if start < end
                && let Some(range_to_copy) = self.buf.get(start..end)
            {
                self.write_all(range_to_copy.to_vec().as_slice())?;
            } else if let Some(start_to_back) = self.buf.get(start..)
                && let Some(front_to_end) = self.buf.get(..end)
            {
                let mut range_to_copy = start_to_back.to_vec();
                range_to_copy.extend_from_slice(front_to_end);
                self.write_all(&range_to_copy)?;
            }

            bytes_written += (end + WINDOW_SIZE - start) % WINDOW_SIZE;
        }

        Ok(())
    }
}

impl<W: Write> Write for CurrentCachedWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = self.main_stream.write(buf)?;

        let end = self.write_index + written;

        if end < WINDOW_SIZE
            && let Some(buffer) = self.buf.get_mut(self.write_index..end)
            && let Some(new_data) = buf.get(..written)
        {
            buffer.copy_from_slice(new_data);
        } else if let Ok([buffer1, buffer2]) = self
            .buf
            .get_disjoint_mut([self.write_index..WINDOW_SIZE, 0..(end % WINDOW_SIZE)])
            && let Some(new_data1) = buf.get(..(WINDOW_SIZE - self.write_index))
            && let Some(new_data2) = buf.get((WINDOW_SIZE - self.write_index)..written)
        {
            buffer1.copy_from_slice(new_data1);
            buffer2.copy_from_slice(new_data2);
        }

        self.write_index = self.write_index.saturating_add(written) % WINDOW_SIZE;

        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.main_stream.flush()
    }
}

// ==========================================
// Playground impl
// ==========================================
pub struct OptCachedWriter<W> {
    main_stream: W,
    buf: Box<[u8; WINDOW_SIZE]>,
    write_index: usize,
}

impl<W: Write> OptCachedWriter<W> {
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

    pub fn repeat_from(&mut self, distance: usize, length: usize) -> Result<()> {
        if distance > WINDOW_SIZE {
            bail!("Trying to read too far back in LZ77 decoding");
        }

        let mut bytes_written = 0;

        while bytes_written != length {
            let start = self
                .write_index
                .saturating_add(WINDOW_SIZE)
                .saturating_sub(distance)
                % WINDOW_SIZE;

            let offset = usize::min(distance, length.saturating_sub(bytes_written));
            let end = start.saturating_add(offset) % WINDOW_SIZE;

            if start < end
                && let Some(range_to_copy) = self.buf.get(start..end)
            {
                self.write_all(range_to_copy.to_vec().as_slice())?;
            } else if let Some(start_to_back) = self.buf.get(start..)
                && let Some(front_to_end) = self.buf.get(..end)
            {
                let mut range_to_copy = start_to_back.to_vec();
                range_to_copy.extend_from_slice(front_to_end);
                self.write_all(&range_to_copy)?;
            }

            bytes_written += (end + WINDOW_SIZE - start) % WINDOW_SIZE;
        }

        Ok(())
    }
}

impl<W: Write> Write for OptCachedWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = self.main_stream.write(buf)?;

        let end = self.write_index + written;

        if end < WINDOW_SIZE
            && let Some(buffer) = self.buf.get_mut(self.write_index..end)
            && let Some(new_data) = buf.get(..written)
        {
            buffer.copy_from_slice(new_data);
        } else if let Ok([buffer1, buffer2]) = self
            .buf
            .get_disjoint_mut([self.write_index..WINDOW_SIZE, 0..(end % WINDOW_SIZE)])
            && let Some(new_data1) = buf.get(..(WINDOW_SIZE - self.write_index))
            && let Some(new_data2) = buf.get((WINDOW_SIZE - self.write_index)..written)
        {
            buffer1.copy_from_slice(new_data1);
            buffer2.copy_from_slice(new_data2);
        }

        self.write_index = self.write_index.saturating_add(written) % WINDOW_SIZE;

        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.main_stream.flush()
    }
}

// ==========================================
// benchmarks
// ==========================================
fn bench_repeat_from(c: &mut Criterion) {
    let mut group = c.benchmark_group("LZ77 repeat_from (4KB chunk)");

    let length = 4096;
    let distance = 256;
    group.throughput(Throughput::Bytes(length as u64));

    group.bench_function("Original Writer", |b| {
        let mut writer = CurrentCachedWriter::new(sink());
        writer.write_all(&[0x42; 512]).unwrap();

        b.iter(|| {
            writer
                .repeat_from(black_box(distance), black_box(length))
                .unwrap();
        });
    });

    group.bench_function("Optimized Writer", |b| {
        let mut writer = OptCachedWriter::new(sink());
        writer.write_all(&[0x42; 512]).unwrap();

        b.iter(|| {
            writer
                .repeat_from(black_box(distance), black_box(length))
                .unwrap();
        });
    });

    group.finish();
}

criterion_group!(benches, bench_repeat_from);
criterion_main!(benches);
