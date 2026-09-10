//! The entry point and the CLI
//!
//! This module is responsible for
//! * parsing the CLI arguments
//! * initializing the output stream
//! * actually calling the DEFLATE algorithm
//!
//! ## CLI
//!
//! The CLI structure of puncture closely mimics the standard `gzip`/`pigz` CLI.
use std::{
    fs::File,
    io::{self, BufRead, BufReader, Write},
    path::PathBuf,
};

use clap::Parser;

use puncture::{args::CliArgs, compressor::Compressor, extraction::Extractor};

fn main() -> Result<(), io::Error> {
    let args = CliArgs::parse();

    for file in &args.files {
        let input_stream: Box<dyn BufRead> = if file == "-" {
            Box::new(BufReader::with_capacity(1024 * 1024, std::io::stdin()))
        } else {
            Box::new(BufReader::with_capacity(1024 * 1024, File::open(file)?))
        };

        if args.decompress {
            run_extraction(&args, file, input_stream)?;
        } else {
            run_compression(&args, file, input_stream)?;
        }

        if file != "-" && !args.to_stdout && !args.keep_input {
            std::fs::remove_file(file)?;
        }
    }

    Ok(())
}

fn run_extraction<R: BufRead>(args: &CliArgs, file: &str, input_stream: R) -> io::Result<()> {
    let mut ext = Extractor::new(input_stream);

    let header = ext.process_header()?;

    let mut output_stream: Box<dyn Write> = if args.to_stdout || file == "-" {
        Box::new(std::io::stdout())
    } else {
        let file_name = header.file_name.map_or_else(
            || Ok(String::from(file)),
            |def| -> io::Result<String> {
                let embedded_name = def
                    .into_string()
                    .map_err(|_| io::Error::other("Original file name isn't valid UTF8"))?;

                Ok(format!("{embedded_name}.gz"))
            },
        )?;

        let path = PathBuf::from(file_name).with_extension("");
        Box::new(if args.force {
            File::create(path)?
        } else {
            File::create_new(path)?
        })
    };

    ext.deflate(&mut output_stream)
}

fn run_compression<R: BufRead>(args: &CliArgs, file: &str, mut input_stream: R) -> io::Result<()> {
    let output_stream: Box<dyn Write> = if args.to_stdout || file == "-" {
        Box::new(std::io::stdout())
    } else {
        let path = PathBuf::from(file).with_added_extension("gz");
        Box::new(if args.force {
            File::create(path)?
        } else {
            File::create_new(path)?
        })
    };

    let mut compr = Compressor::new(output_stream, args.compr_lvl.to_max_chain());

    compr.write_header()?;

    // Stream the contents of the input_stream through the `Compressor`,
    // running the compression algorithm
    std::io::copy(&mut input_stream, &mut compr)?;

    compr.finish()?;

    Ok(())
}
