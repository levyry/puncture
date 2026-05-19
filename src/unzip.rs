use std::{
    fs::File,
    io::{self, BufReader, BufWriter},
    path::Path,
};

use crate::extraction::Extraction;

pub fn extract(path: &Path, output: Option<&Path>) -> Result<(), io::Error> {
    // TODO: For now, we read the whole file into memory. This will
    // cause an OOM for large files. Later on, there should be a CLI option
    // for configuring whether to read the whole file into memory, or
    // keep reading from the file itself.
    let compressed_data = std::fs::read(path)?;

    // Try creating output before doing any work
    let new_file_path = output.unwrap_or_else(|| {
        path.file_stem()
            .map(Path::new)
            .expect("Somehow there was no file name")
    });

    let output_file = File::create_new(new_file_path)?;
    let mut uncompressed_output = BufWriter::new(output_file);

    let _ext = Extraction::new(
        BufReader::new(compressed_data.as_slice()),
        &mut uncompressed_output,
    );

    Ok(())
}

struct HeaderInfo {
    flags: u8,
    mtime: u32,
    crc32: u32,
    crc16: u16,
    isize: u32,
}
