//! # Puncture
//! A minimal, educational, and reasonably fast gzip decompressor.
//!
//! This library provides the underlying DEFLATE extraction tools used by the
//! `puncture` CLI.
//!
//! The primary entry points are the [`bitreader::BitReader`] and the [`extraction::Extractor`].

use std::ffi::CString;

pub mod args;
pub mod bitreader;
pub mod bitwriter;
pub mod cached_writer;
pub mod compressor;
pub mod extraction;

// TODO: Add more headers
#[derive(Debug, Default)]
pub struct GzipHeader {
    /// The original file name in the GZIP header, if it is present.
    pub file_name: Option<CString>,

    /// The most recent modification time of the original file in Unix format
    pub mtime: u32,

    pub xfl: u8,

    pub os: u8,
}
