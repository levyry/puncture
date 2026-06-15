# puncture

[![Casual Maintenance Intended](https://casuallymaintained.tech/badge.svg)](https://casuallymaintained.tech/)

a minimal implementation of a gzip decompressor

## installation

using `cargo`:

```bash
cargo install puncture
```

## usage

to decompress a file, replacing it with the original uncompressed version:

```bash
puncture -d /path/to/compressed_file.gz
```

to decompress a file, writing the contents to standard out:

```bash
puncture -cd /path/to/compressed_file.gz
```

to decompress a file, keeping the original uncompressed version and specifying the output filename:

```bash
puncture -cdk /path/to/compressed_file.gz > path/to/uncompressed_file
```

## philosophy

i wanted to create a project that tries to strike the middle ground between two extremes: it tries to be simple (but not the simplest) and fast (but not the fastest).

there are certainly simpler and shorter gzip implementations, but these projects are usually so stripped down, that they sacrifice educational value for simplicity. there isn't a lot to be learned from a 280 line program for someone who is already familiar with the topic.

there are also certainly faster, and more complicated implementations. these are usually so heavily optimized, that code readability suffers as a result. there _are_ things to be learnt from these projects as well, but their purpose is very different.

this project tries to be faster than naive implementations, but not so heavily optimized, that you need to spend hours to decipher what sort of bit-magic is happening. this way hopefully as many people as possible can take something away from this project.

since the primary goal of this project is to provide educational value, i tried to keep the codebase as simple as I could:

* [`src/bitreader.rs`](https://github.com/levyry/puncture/blob/main/src/bitreader.rs): a custom LSB-first bit reader. it maintains a `u128` internal cache to allow peeking and consuming a specific number of bits at a time.
* [`src/cached_writer.rs`](https://github.com/levyry/puncture/blob/main/src/cached_writer.rs): manages the split 32 KB sliding window history required by LZ77 and acts as a buffer, ensuring we do as few underlying `write` system calls as possible while also calculating the required CRC32 checksums.
* [`src/extraction.rs`](https://github.com/levyry/puncture/blob/main/src/extraction.rs): this is where the main DEFLATE algorithm lives: it decodes the dynamic Huffman trees, builds the lookup tables, and executes the main decompression loop. the GZIP header parsing also lives here.
* [`src/main.rs`](https://github.com/levyry/puncture/blob/main/src/main.rs): the CLI harness, responsible for calling the main DEFLATE algorithm with the correct inputs and flags.

i tried to use as few dependencies as possible:

* [`clap`](https://github.com/clap-rs/clap) for command line argument parsing
* [`crc32fast`](<https://github.com/srijs/rust-crc32fast>) for calculating the crc32 checksum

## resources

i mainly used [RFC1952](https://datatracker.ietf.org/doc/html/rfc1952) for the GZIP header parsing, and [RFC1951](https://datatracker.ietf.org/doc/html/rfc1951) for the actual DEFLATE algorithm. other notable resources that helped me include:

* [`infgen`](https://github.com/madler/infgen) for viewing deflate streams with semantic information
* [An Explanation of the Deflate Algorithm](https://zlib.net/feldspar.html) by Anteus Feldspar
* [this random youtube video (goated)](https://www.youtube.com/watch?v=cYHK0VM1fBg)

## statistics

tl;dr: `gzip`/zlib is roughly 1.3x faster, but has a 3x larger codebase dedicated to decompressing

### speed

i used the [Silesia Open Source Compression Benchmark](https://mattmahoney.net/dc///silesia.html) as the input data, and [`hyperfine`](https://github.com/sharkdp/hyperfine) as the benchmark harness.

```bash
hyperfine --warmup 5 --min-runs 10 \
  "puncture -cdk ./silesia.tar.gz > /dev/null" \
  "gzip -cdk ./silesia.tar.gz > /dev/null"

Benchmark 1: puncture -cdk ./silesia.tar.gz > /dev/null
  Time (mean ± σ):     797.0 ms ±   5.9 ms    [User: 778.9 ms, System: 11.0 ms]
  Range (min … max):   788.2 ms … 806.2 ms    10 runs
 
Benchmark 2: gzip -cdk ./silesia.tar.gz > /dev/null
  Time (mean ± σ):     600.4 ms ±   4.9 ms    [User: 588.9 ms, System: 5.0 ms]
  Range (min … max):   593.6 ms … 609.9 ms    10 runs
 
Summary
  gzip -cdk ./silesia.tar.gz > /dev/null ran
    1.33 ± 0.01 times faster than puncture -cdk ./silesia.tar.gz > /dev/null
```

so `gzip` is roughly 1.33x faster. other online toy implementations are usually up to 2x-3x times slower than `gzip`.

### codebase

the `gzip` binary uses the `zlib` library under the hood, so i will compare against that. the part of `zlib` responsible for decompression is roughly 2000 lines of code, while this implementation uses roughly 650 lines of code, which is a ~3x reduction in size.

additionally, this implementations uses some pretty high level abstractions for this low-level of a program (check out [`bitreader.rs`](https://github.com/levyry/puncture/blob/main/src/bitreader.rs)), and purposefully forgoes implementing some of the more hard to understand optimizations. there is no unsafe or any inline assembly.

### complexity

* creates LUTs for dynamic huffman trees
* use a split 64 KB linear buffer for the 32 KB LZ77 sliding window and the writing buffer
* handles overlapping LZ77 matches using an exponential doubling algorithm

there are some optimizations i didn't end up implementing, such as:

* two-tier LUTs for huffman codes
* inlining the bit reading logic to extraction

with these implementations, the performance should be completely on-par with `gzip`.

## license

licensed under either of:

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   <https://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or
   <https://opensource.org/license/mit>)

at your option.
