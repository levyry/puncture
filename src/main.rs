use std::{env, path::Path};

mod bitreader;
mod crc32;
mod extraction;
mod unzip;
mod zip;

fn main() -> Result<(), anyhow::Error> {
    let args: Vec<String> = env::args().collect();

    match args.len() {
        2 if let Some(cmd) = args.get(1) => match cmd.as_str() {
            "-v" | "--version" => show_version(),
            _ => show_help(),
        },
        _ if let Some(cmd) = args.get(1)
            && let Some(path) = args.get(2)
            && let output = args.get(3) =>
        {
            match cmd.as_str() {
                "-e" | "--extract" => unzip::extract(Path::new(path), output.map(Path::new))?,
                "-a" | "--archive" => zip::archive(Path::new(path), output.map(Path::new))?,
                _ => show_help(),
            }
        }
        _ => show_help(),
    }

    Ok(())
}

fn show_version() {
    println!("mini-gzip 0.1.0");
}

fn show_help() {
    println!(
        "Usage: mini-gzip [OPTION]... [FILE]... [OUTPUT]...
Compress or uncompress FILEs using the gzip format.

If no OUTPUT is provided, the original FILE name is going to be used.
    
Options:
    
-e, --extract extract a FILE into OUTPUT
-a, --archive create a new gzip archive from a FILE as OUTPUT
-h, --help    show this help message
-v, --version display version number"
    );
}
