//! # Puncture
//! A minimal, educational, and reasonably fast gzip decompressor.
//!
//! This library provides the underlying DEFLATE extraction tools used by the
//! `puncture` CLI.
//!
//! The primary entry points are the [`bitreader::BitReader`] and the [`extraction::Extractor`].

pub mod bitreader;
pub mod cached_writer;
pub mod extraction;