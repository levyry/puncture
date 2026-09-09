#![expect(clippy::unwrap_used, clippy::panic_in_result_fn)]

use assert_cmd::Command;
use std::{fs, io};

fn roundabout(input: &str) -> io::Result<()> {
    let compress_cmd = Command::cargo_bin("puncture")
        .unwrap()
        .args(["-c", "-k", input])
        .assert()
        .success();

    let decompress_cmd = Command::cargo_bin("puncture")
        .unwrap()
        .args(["-d", "-c", "-k"])
        .write_stdin(compress_cmd.get_output().stdout.clone())
        .assert()
        .success();

    let actual = &decompress_cmd.get_output().stdout;
    let expected = fs::read(input)?;

    assert!(
        actual == &expected,
        "Roundabout output for '{input}' failed. Expected length: {} Actual length: {}",
        expected.len(),
        actual.len(),
    );

    Ok(())
}

#[test]
fn test_roundabout_random() -> io::Result<()> {
    roundabout("tests/data/original/large_random.txt")
}

#[test]
fn test_roundabout_shakespeare() -> io::Result<()> {
    roundabout("tests/data/original/shakespeare.txt")
}
