//! The entry point and the CLI
//!
//! This module is responsible for
//! * parsing the CLI arguments
//! * initializing the output stream
//! * actually calling the DEFLATE algorithm
//!
//! ## CLI
//!
//! The CLI structure of puncture closely mimics the standard `gzip`/`pigz` CLI,
//! see [`get_cli_args`] for more information.
use std::{
    fs::File,
    io::{self, BufRead, BufReader, IsTerminal, Write},
    path::Path,
};

use clap::{Arg, ArgAction, ArgMatches, Command};

use puncture::{bitreader::BitReader, extraction::Extractor};

fn main() -> Result<(), io::Error> {
    let args = get_cli_args()?;

    let is_decompress = args.get_flag("decompress");
    let to_stdout = args.get_flag("stdout");
    let keep = args.get_flag("keep");

    let files: Vec<&str> = args
        .get_many::<String>("FILES")
        .unwrap_or_default()
        .map(String::as_str)
        .collect();

    let files = if files.is_empty() { vec!["-"] } else { files };

    for file in files {
        if is_decompress {
            if file == "-" {
                let input_stream =
                    Box::new(BufReader::with_capacity(1024 * 1024, std::io::stdin()));

                run_extraction(to_stdout, file, input_stream)?;
            } else {
                let input_stream =
                    Box::new(BufReader::with_capacity(1024 * 1024, File::open(file)?));

                run_extraction(to_stdout, file, input_stream)?;

                if !to_stdout && !keep {
                    std::fs::remove_file(file)?;
                }
            }
        } else {
            eprintln!("Compression is not yet implemented. Use -d to decompress.");
            std::process::exit(1);
        }
    }

    Ok(())
}

/// Returns the parsed CLI arguments
///
/// The arguments are:
///
/// * `-d` for decompression
/// * `-c` for routing to stdout
/// * `-k` for keeping the original file after decompression
/// * `-h` for printing the help message
/// * `-V` for printing the version
///
/// It can process multiple files at a time, and prints help if no arguments are
/// provided.
fn get_cli_args() -> io::Result<ArgMatches> {
    let mut cmd = Command::new("puncture")
        .version("0.1.0")
        .about("Compress or uncompress files using the gzip format.")
        .arg(
            Arg::new("decompress")
                .short('d')
                .long("decompress")
                .help("decompress")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("stdout")
                .short('c')
                .long("stdout")
                .help("write on standard output, keep original files unchanged")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("keep")
                .short('k')
                .long("keep")
                .help("keep (don't delete) input files")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("FILES")
                .help("FILEs to process. With no FILE, or when FILE is -, read standard input.")
                .action(ArgAction::Append)
                .num_args(1..),
        );

    if std::env::args().len() == 1 && std::io::stdin().is_terminal() {
        cmd.print_help()?;
        std::process::exit(0);
    }

    Ok(cmd.get_matches())
}

fn get_output_stream(
    print_to_stdout: bool,
    file: &str,
    extractor: &Extractor<'_, impl BufRead>,
) -> io::Result<Box<dyn Write>> {
    Ok(if print_to_stdout || file == "-" {
        Box::new(std::io::stdout())
    } else if let Some(original_file_name) = extractor.get_file_name() {
        let name = original_file_name
            .clone()
            .into_string()
            .map_err(|_| io::Error::other("Original file name isn't valid UTF8"))?;

        Box::new(File::create_new(name)?)
    } else {
        let path = Path::new(file);
        let stem = path
            .file_stem()
            .map(Path::new)
            .expect("Invalid file name provided");

        Box::new(File::create_new(stem)?)
    })
}

fn run_extraction<R: BufRead>(to_stdout: bool, file: &str, input_stream: R) -> io::Result<()> {
    let mut br = BitReader::new(input_stream);
    let mut ext = Extractor::new(&mut br);

    ext.process_header();

    let mut output_stream = get_output_stream(to_stdout, file, &ext)?;

    ext.deflate(&mut output_stream)
}
