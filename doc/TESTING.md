# Unit tests

Unit tests are directly part of the codebase as standard Rust `#[test]` blocks.
They test the input/output behavior of individual parsing and printing functions
(up to the toplevel functions handling complete emails).

To run the testsuite:
```
cargo test
```

# Fuzzing

We use fuzzing to test a *roundtrip property* of the combined parser, printer and AST.
Specifically, we check that: generating an arbitrary email AST, printing it, then parsing it back, yields an equivalent AST.

This ensures that the parser and printer are consistent with each other; it also ensures that every email value that can be represented by the `eml-codec` AST is correctly handled by the parser and printer.

See [fuzz/README.md](../fuzz/README.md) for practical details on how to run the fuzz targets.

# Tracing parser recovery decisions on real-world emails

We provide tracing instrumentation in `eml-codec` and a tool to collect traces generated when parsing test email corpuses. This allows testing the parser behavior on real-world emails, which often contain *non-RFC-compliant data*.
This is complementary to our fuzzing methodology, which tests parser-printer consistency on what the internal AST represents, i.e. mostly *RFC-compliant data*.

The `eml-codec` parser never fails when reading its input; instead, it implements *recovery* strategies that allow it to continue and return a best-effort result. In a number of cases, the parser can recover from ill-formed input and interpret it in a plausible fashion.
In the remaining cases where the parser cannot recognize a part of the input, this part is then discarded, allowing parsing to continue.

`eml-codec` provides two optional feature flags that make it *output a trace of the recovery strategies it applied* during parsing (leveraging the [tracing](https://docs.rs/tracing/latest/tracing/) crate):
- `tracing-recover`: emit an event each time a recovery strategy was applied to interpret non-compliant data;
- `tracing-unsupported`: emit an event each time some data could not be interpreted and was discarded as last resort.

Parsing a fully RFC-compliant email should not emit any event. In practice, `tracing-recover` tends to be quite verbose on real-world emails, and using `tracing-unsupported` is more useful to detect occurrences of real-world syntax that could possibly be handled better by the parser.

We provide a standalone `trace` tool that runs the parser on a collection of emails and outputs the corresponding tracing events. See the [trace](../examples/README.md#trace-tracing-parser-recovery-strategies-on-email-collections) documentation for more info on using this tool.

See also the [`resources/`](../resources) directory, which documents how to run this tool on some public email corpuses and how to interpret the results.
