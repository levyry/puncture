use std::{
    fs::File,
    io::{BufRead, BufReader, BufWriter, IsTerminal, Write},
    path::Path,
};

use anyhow::{Context, Result};
use clap::{Arg, ArgAction, ArgMatches, Command};

use crate::{bitreader::BitReader, extraction::Extractor};

mod bitreader;
mod crc32;
mod extraction;

fn main() -> Result<(), anyhow::Error> {
    let args = get_cli_args()?;

    let is_extract = args.get_flag("extract");
    let _is_archive = args.get_flag("archive");
    let to_stdout = args.get_flag("stdout");

    let input = args.get_one::<String>("INPUT");
    let output = args.get_one::<String>("OUTPUT");

    let input_stream: Box<dyn BufRead> = if let Some(input_path) = input
        && input_path != "-"
    {
        Box::new(BufReader::new(File::open(input_path)?))
    } else {
        Box::new(BufReader::new(std::io::stdin()))
    };

    if is_extract {
        let mut br = BitReader::new(input_stream);
        let mut ext = Extractor::new(&mut br);

        ext.process_header()?;

        let mut output_stream = get_output_stream(to_stdout, input, output, &ext)?;

        ext.deflate(&mut output_stream)?;
    }

    Ok(())
}

fn get_output_stream(
    print_to_stdout: bool,
    cli_input: Option<&String>,
    cli_output: Option<&String>,
    extractor: &Extractor<'_, impl BufRead>,
) -> Result<Box<dyn Write>> {
    Ok(if print_to_stdout {
        Box::new(BufWriter::new(std::io::stdout()))
    } else if let Some(requested_file_name) = cli_output {
        Box::new(BufWriter::new(File::create_new(requested_file_name)?))
    } else if let Some(original_file_name) = extractor.get_file_name() {
        let name = original_file_name.clone().into_string()?;
        Box::new(BufWriter::new(File::create_new(name)?))
    } else if let Some(input_path) = cli_input
        && input_path != "-"
    {
        Box::new(BufWriter::new(File::create_new(
            Path::new(&input_path)
                .file_stem()
                .map(Path::new)
                .context("Somehow there was no file name")?,
        )?))
    } else {
        // If the stdout flag wasn't given, there was no explicit output and no explicit input, we'll use a placeholder
        Box::new(BufWriter::new(File::create_new("decompressed.txt")?))
    })
}

fn get_cli_args() -> Result<ArgMatches> {
    let mut cmd = Command::new("mini-gzip")
        .version("0.1.0")
        .about("Compress or uncompress files using the gzip format.")
        .arg(
            Arg::new("extract")
                .short('e')
                .long("extract")
                .help("Set mode to extract")
                .action(ArgAction::SetTrue)
                .conflicts_with("archive"),
        )
        .arg(
            Arg::new("archive")
                .short('a')
                .long("archive")
                .help("Set mode to archive (compress)")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("INPUT")
                .help("The input file to process, defaults to stdin if omitted")
                .required(false)
                .index(1),
        )
        .arg(
            Arg::new("OUTPUT")
                .help("Where to extract the files")
                .required(false)
                .index(2),
        )
        .arg(
            Arg::new("stdout")
                .short('s')
                .long("stdout")
                .help("Routes output to standard out")
                .action(ArgAction::SetTrue),
        );

    if std::env::args().len() == 1 && std::io::stdin().is_terminal() {
        cmd.print_help()?;
        #[expect(
            clippy::exit,
            reason = "This function is only called at the beginning of main"
        )]
        std::process::exit(0);
    }

    Ok(cmd.get_matches())
}
