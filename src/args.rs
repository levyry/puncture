use clap::{ArgAction, Parser};

#[derive(Debug, Parser)]
#[command(version, about, long_about = None, next_display_order = None, disable_help_flag = true, disable_version_flag = true)]
pub struct CliArgs {
    /// write on standard output, keep original files unchanged
    #[arg(short = 'c', long = "stdout")]
    pub to_stdout: bool,

    /// decompress
    #[arg(short = 'd', long = "decompress")]
    pub decompress: bool,

    /// force overwrite of output file and compress links
    #[arg(short = 'f', long = "force")]
    pub force: bool,

    /// keep (don't delete) input files
    #[arg(short = 'k', long = "keep")]
    pub keep_input: bool,

    /// FILEs to process. with no FILE, or when FILE is -, read standard input
    #[arg(default_value = "-")]
    pub files: Vec<String>,

    #[command(flatten)]
    pub compr_lvl: CompressionLevel,

    #[arg(short = 'h', long = "help", action = ArgAction::Help, help = "give this help")]
    help: Option<bool>,

    #[arg(short = 'V', long = "version", action = ArgAction::Version, help = "display version number")]
    version: Option<bool>,
}

#[derive(Debug, clap::Args)]
#[group(multiple = false)]
pub struct CompressionLevel {
    #[arg(
        short = '1',
        long = "fast",
        help = "compress faster",
        display_order = 1000
    )]
    pub one: bool,
    #[arg(short = '2', hide = true)]
    pub two: bool,
    #[arg(short = '3', hide = true)]
    pub three: bool,
    #[arg(short = '4', hide = true)]
    pub four: bool,
    #[arg(short = '5', hide = true)]
    pub five: bool,
    #[arg(short = '6', hide = true)]
    pub six: bool,
    #[arg(short = '7', hide = true)]
    pub seven: bool,
    #[arg(short = '8', hide = true)]
    pub eight: bool,
    #[arg(
        short = '9',
        long = "best",
        help = "compress better",
        display_order = 1000
    )]
    pub nine: bool,
}

impl CompressionLevel {
    /// Helper to convert the flags into the `max_chain` used. The values
    /// are taken from the zlib source
    #[must_use]
    pub const fn to_max_chain(&self) -> u32 {
        match (
            self.one, self.two, self.three, self.four, self.five, self.seven, self.eight, self.nine,
        ) {
            // Default flag is 6
            (false, false, false, false, false, false, false, false) => 128,
            (true, _, _, _, _, _, _, _) => 4,
            (_, true, _, _, _, _, _, _) => 8,
            (_, _, true, _, _, _, _, _) => 16,
            (_, _, _, true, _, _, _, _) => 32,
            (_, _, _, _, true, _, _, _) => 64,
            (_, _, _, _, _, true, _, _) => 256,
            (_, _, _, _, _, _, true, _) => 1024,
            (_, _, _, _, _, _, _, true) => 4096,
        }
    }
}
