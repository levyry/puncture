# puncture

[![Casual Maintenance Intended](https://casuallymaintained.tech/badge.svg)](https://casuallymaintained.tech/)
[![Released API docs](https://docs.rs/puncture/badge.svg)](https://docs.rs/puncture)

a minimal implementation of a gzip (de)compressor

## installation

using `cargo`:

```bash
cargo install puncture --locked
```

## usage

in general, the flags are comparable with the standard `gzip` utility, except for a few missing ones:

```bash
# decompress a file, replacing it with the original uncompressed version
puncture -d /path/to/compressed_file.gz

# decompress a file, writing the contents to standard out
puncture -dc /path/to/compressed_file.gz

# compress a file, keeping the original and specifying the output filename
puncture -c /path/to/file > path/to/decompressed_file.gz
```

## philosophy

i wanted to create a project that tries to strike the middle ground between two extremes: it tries to be simple (but not the simplest) and fast (but not the fastest).

there are certainly simpler and shorter gzip implementations, but these projects are usually so stripped down, that they sacrifice educational value for simplicity. there isn't a lot to be learned from a 280 line program for someone who is already even slightly familiar with the topic.

there are also certainly faster, and more complicated implementations. these are usually so heavily optimized, that code readability suffers as a result. there _are_ things to be learnt from these projects as well, but their purpose is very different.

this project is mainly for those that want to move past code-golfed implementations, but aren't necessarily ready to dive head first into the zlib codebase (believe me, i tried).

if you are looking for a place to get started, [read the documentation](https://docs.rs/puncture/latest/puncture/) over at docs.rs, i tried to document everything to the best of my abilities (note that the compressor and bitwriter modules aren't documented yet)

## resources

i mainly used [RFC1952](https://datatracker.ietf.org/doc/html/rfc1952) for the GZIP header parsing, and [RFC1951](https://datatracker.ietf.org/doc/html/rfc1951) for the actual DEFLATE algorithm. other notable resources that helped me include:

* [`infgen`](https://github.com/madler/infgen) for viewing deflate streams with semantic information
* [An Explanation of the Deflate Algorithm](https://zlib.net/feldspar.html) by Anteus Feldspar
* [this random youtube video](https://www.youtube.com/watch?v=cYHK0VM1fBg)

## benchmarks

### decompression

i used the [Silesia Open Source Compression Benchmark](https://sun.aei.polsl.pl//~sdeor/index.php?page=silesia) as the input data, and [`hyperfine`](https://github.com/sharkdp/hyperfine) as the benchmark harness.

i ran this command:

```bash
hyperfine --warmup 5 --min-runs 10 \
  "puncture -cdk ./silesia.tar.gz > /dev/null" \
  "gzip -cdk ./silesia.tar.gz > /dev/null"
```

<details>

<summary>Output as of v0.2.0</summary>

```bash
Benchmark 1: puncture -cd ./silesia.tar.gz > /dev/null
  Time (mean ± σ):     867.0 ms ±   7.0 ms    [User: 849.8 ms, System: 9.5 ms]
  Range (min … max):   859.4 ms … 880.5 ms    10 runs
 
Benchmark 2: gzip -cd ./silesia.tar.gz > /dev/null
  Time (mean ± σ):     601.8 ms ±   5.8 ms    [User: 591.2 ms, System: 5.4 ms]
  Range (min … max):   594.0 ms … 611.0 ms    10 runs
 
Summary
  gzip -cd ./silesia.tar.gz > /dev/null ran
    1.44 ± 0.02 times faster than puncture -cd ./silesia.tar.gz > /dev/null
```

</details>

tl;dr `gzip` is roughly 1.44x faster. to my knowledge, other "toy" implementations online are usually in the 2-3x range

### compression

<details>

<summary>Summary table as of v0.2.0</summary>

| command | mean compression time (s) | compression ratio (%) |
| --------- | ------------------ | ------------------- |
| gzip -9c ./silesia.tar | 16.575 | 68.1% |
| gzip -6c ./silesia.tar | 6.961 | 67.8% |
| gzip -1c ./silesia.tar | 2.230 | 63.5% |
| puncture -9c ./silesia.tar | 8.587 | 62.2% |
| puncture -6c ./silesia.tar | 5.054 | 62.0% |
| puncture -1c ./silesia.tar | 2.549 | 58.8% |

</details>

for compression there are multiple angles: speed and compression ratio. for compression speed, i ran this command:

```bash
hyperfine --warmup 3 --min-runs 10 \
  "puncture -1c ./silesia.tar > /dev/null" \
  "gzip -1c ./silesia.tar > /dev/null" \
  "puncture -c ./silesia.tar > /dev/null" \
  "gzip -c ./silesia.tar > /dev/null" \
  "puncture -9c ./silesia.tar > /dev/null" \
  "gzip -9c ./silesia.tar > /dev/null"
```

<details>

<summary>Output as of v0.2.0</summary>

```bash
Benchmark 1: puncture -1c ./silesia.tar > /dev/null
  Time (mean ± σ):      2.549 s ±  0.025 s    [User: 2.413 s, System: 0.108 s]
  Range (min … max):    2.509 s …  2.584 s    10 runs
 
Benchmark 2: gzip -1c ./silesia.tar > /dev/null
  Time (mean ± σ):      2.230 s ±  0.019 s    [User: 2.178 s, System: 0.021 s]
  Range (min … max):    2.209 s …  2.261 s    10 runs
 
Benchmark 3: puncture -c ./silesia.tar > /dev/null
  Time (mean ± σ):      5.054 s ±  0.015 s    [User: 4.926 s, System: 0.093 s]
  Range (min … max):    5.035 s …  5.078 s    10 runs
 
Benchmark 4: gzip -c ./silesia.tar > /dev/null
  Time (mean ± σ):      6.961 s ±  0.011 s    [User: 6.891 s, System: 0.024 s]
  Range (min … max):    6.943 s …  6.974 s    10 runs
 
Benchmark 5: puncture -9c ./silesia.tar > /dev/null
  Time (mean ± σ):      8.587 s ±  0.058 s    [User: 8.411 s, System: 0.105 s]
  Range (min … max):    8.506 s …  8.667 s    10 runs
 
Benchmark 6: gzip -9c ./silesia.tar > /dev/null
  Time (mean ± σ):     16.575 s ±  0.036 s    [User: 16.392 s, System: 0.035 s]
  Range (min … max):   16.510 s … 16.623 s    10 runs
 
Summary
  gzip -1c ./silesia.tar > /dev/null ran
    1.14 ± 0.01 times faster than puncture -1c ./silesia.tar > /dev/null
    2.27 ± 0.02 times faster than puncture -c ./silesia.tar > /dev/null
    3.12 ± 0.03 times faster than gzip -c ./silesia.tar > /dev/null
    3.85 ± 0.04 times faster than puncture -9c ./silesia.tar > /dev/null
    7.43 ± 0.06 times faster than gzip -9c ./silesia.tar > /dev/null
```

</details>

to check the compression ratios, i compressed `silesia.tar` at different levels with both programs, and then ran

```bash
gzip --list *.gz
```

<details>

<summary>Output as of v0.2.0</summary>

```bash
         compressed        uncompressed  ratio uncompressed_name
           77388507           211948544  63.5% ./gzip1-silesia.tar
           68236769           211948544  67.8% ./gzip6-silesia.tar
           67649924           211948544  68.1% ./gzip9-silesia.tar
           87310155           211948544  58.8% ./punc1-silesia.tar
           80547504           211948544  62.0% ./punc6-silesia.tar
           80205849           211948544  62.2% ./punc9-silesia.tar
          461338708          1271691264  63.7% (totals)
```

</details>

these results are somewhat expected, as i only implented fixed huffman encoding, and my LZ77 pattern matching is eager as well.

### complexity

* creates LUTs for dynamic huffman trees
* use a split 64 KB linear buffer for the 32 KB LZ77 sliding window and the writing buffer
* handles overlapping LZ77 matches using an exponential doubling algorithm
* only uses fixed huffman trees for compression instead of dynamic tree generation

there are some optimizations i didn't end up implementing, such as:

* two-tier LUTs for huffman codes
* inlining the bit reading logic to extraction
* lazy LZ77 pattern matching

## license

licensed under either of:

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   <https://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or
   <https://opensource.org/license/mit>)

at your option.
