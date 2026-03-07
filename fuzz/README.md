# eml-codec fuzz testing

## Setup

Cargo fuzz requires nightly and `cargo-fuzz`. Instal via:

``` sh
rustup install nightly
cargo install cargo-fuzz
```

## Running a fuzz target

Running a fuzz target:

``` sh
./fuzz.sh <fuzz target>
```

Available fuzz targets (can be listed using `cargo fuzz list`):
- `message_print_parse`: a target that generates an arbitrary email message
  (driven by the fuzzer), prints it, parses the result, and checks that the
  parsed message is equivalent to the original one. This allows catching
  inconsistencies between parser, printer, and invariants of the library AST
  types.
- `message_parse`: a target that generates raw bytes (driven by the fuzzer) and
  parses them as an email message. This checks that parsing never crashes
  (panics, stack overflow, timeout) on unknown inputs.

**The expectation is that these targets must not fail.** If you find a crash,
please fill a bug report!

## Known issues

- We have not yet observed this when performing fuzzing, but parsing of a deeply
  nested multipart message can likely crash the parser with a stack overflow
  ([Tracking issue](https://git.deuxfleurs.fr/Deuxfleurs/eml-codec/issues/38)).
