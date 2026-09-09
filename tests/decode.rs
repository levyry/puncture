#![expect(clippy::unwrap_used, clippy::panic_in_result_fn)]

use assert_cmd::Command;
use std::{fs, io};

fn run_decompression(archive_path: &str, expected_path: &str) -> io::Result<()> {
    let cmd = Command::cargo_bin("puncture")
        .unwrap()
        .args(["-d", "-c", "-k", archive_path])
        .assert()
        .success();

    let actual = &cmd.get_output().stdout;
    let expected = fs::read(expected_path)?;

    assert!(
        actual == &expected,
        "Decompressed output for '{archive_path}' did not match the original file '{expected_path}'. Expected length: {} Actual length: {}",
        expected.len(),
        actual.len(),
    );

    Ok(())
}

#[test]
fn test_dynamic_huffman_random() -> io::Result<()> {
    run_decompression(
        "tests/data/dynamic/large_random.txt.gz",
        "tests/data/original/large_random.txt",
    )
}

#[test]
fn test_dynamic_huffman_shakespeare() -> io::Result<()> {
    run_decompression(
        "tests/data/dynamic/shakespeare.txt.gz",
        "tests/data/original/shakespeare.txt",
    )
}

#[test]
fn test_fixed_huffman_random() -> io::Result<()> {
    run_decompression(
        "tests/data/fixed/large_random.txt.gz",
        "tests/data/original/large_random.txt",
    )
}

#[test]
fn test_fixed_huffman_shakespeare() -> io::Result<()> {
    run_decompression(
        "tests/data/fixed/shakespeare.txt.gz",
        "tests/data/original/shakespeare.txt",
    )
}

#[test]
fn test_no_compr_random() -> io::Result<()> {
    run_decompression(
        "tests/data/nocompr/large_random.txt.gz",
        "tests/data/original/large_random.txt",
    )
}

#[test]
fn test_no_compr_shakespeare() -> io::Result<()> {
    run_decompression(
        "tests/data/nocompr/shakespeare.txt.gz",
        "tests/data/original/shakespeare.txt",
    )
}
