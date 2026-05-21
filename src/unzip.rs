use std::{
    fs::File,
    io::BufRead,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use crate::extraction::{Extraction, ProcessedHeader};

pub fn extract(path: &Path, output: Option<&Path>) -> Result<()> {
    // TODO: For now, we read the whole file into memory. This will
    // cause an OOM for large files. Later on, there should be a CLI option
    // for configuring whether to read the whole file into memory, or
    // keep reading from the file itself, or stdin, etc.
    let compressed_data = std::fs::read(path)?;

    let ext = Extraction::new(compressed_data.as_slice());

    let ext = ext.process_header()?;

    // TODO: For now, we write only to the file on disk. This is a bit
    // slow, but safer in case of very large files. Later on, there
    // should be a CLI option for configuring whether to write into memory,
    // disk, or to something like stdout, etc.
    let mut output_file = create_output_file(path, output, &ext)?;

    ext.extract_into(&mut output_file)?;

    Ok(())
}

fn create_output_file(
    input: &Path,
    output: Option<&Path>,
    ext: &Extraction<impl BufRead, ProcessedHeader>,
) -> Result<File, anyhow::Error> {
    let new_file_path = match output {
        Some(path) => path,
        None if let Some(original_name) = ext.get_file_name() => {
            let string = original_name.clone().into_string()?;
            &PathBuf::from(string)
        }
        None => input
            .file_stem()
            .map(Path::new)
            .context("Somehow there was no file name")?,
    };

    Ok(File::create_new(new_file_path)?)
}
