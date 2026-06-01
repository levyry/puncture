use puncture::bitreader::BitReader;
use puncture::extraction::Extractor;
use std::fs;
use std::io::Cursor;

#[test]
#[expect(clippy::expect_used)]
fn test_fixed_huffman_random() {
    // Arrange
    let archive_path = "tests/data/large_random.txt.gz";
    let expected_text_path = "tests/data/large_random.txt";

    let compressed_data = fs::read(archive_path).expect("Failed to read compressed file");
    let expected_output = fs::read(expected_text_path).expect("Failed to read expected text file");

    let cursor = Cursor::new(compressed_data);
    let mut br = BitReader::new(cursor);
    let mut ext = Extractor::new(&mut br);

    ext.process_header().expect("Failed to process gzip header");

    // Act
    let mut output_buffer = Vec::new();
    ext.deflate(&mut output_buffer)
        .expect("Failed to deflate payload");

    // Assert
    assert_eq!(
        output_buffer, expected_output,
        "Decompressed buffer did not match expected output!"
    );
}

#[test]
#[expect(clippy::expect_used)]
fn test_fixed_huffman_shakespeare() {
    // Arrange
    let archive_path = "tests/data/shakespeare.txt.gz";
    let expected_text_path = "tests/data/shakespeare.txt";

    let compressed_data = fs::read(archive_path).expect("Failed to read compressed file");
    let expected_output = fs::read(expected_text_path).expect("Failed to read expected text file");

    let cursor = Cursor::new(compressed_data);
    let mut br = BitReader::new(cursor);
    let mut ext = Extractor::new(&mut br);

    ext.process_header().expect("Failed to process gzip header");

    // Act
    let mut output_buffer = Vec::new();
    ext.deflate(&mut output_buffer)
        .expect("Failed to deflate payload");

    // Assert
    assert_eq!(
        output_buffer, expected_output,
        "Decompressed buffer did not match expected output!"
    );
}
