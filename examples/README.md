# Example programs and tools

## `simple`: a demo program

The `simple` example program illustrates basic use of the `eml-codec` library.

## `eml_parse`: parsing and reprinting tool 

The `eml_parse` tool allows testing parsing and printing of a full email. The
binary parses its standard input as an email, and reprints the parsed email on
its standard output. When passed the `--show-ast` option, it also prints a
debugging view of the internal `eml-codec` AST on the standard error output. The
printed email is guaranteed to be RFC compliant (TODO: be more precise); parts of the input using
obsolete or invalid syntax will be reprinted using valid syntax when possible,
or dropped otherwise.

Usage example:
```shell
$ cargo run --bin eml_parse -- --show-ast <<EOF
hello: barrr
date: uhh

hello??
EOF
```

outputs:

```shell
--- message structure ---
Message {
    imf: Imf {
        date: 1970-01-01T00:00:00+00:00,
        from: Single {
            from: MailboxRef {
                addrspec: AddrSpec(
                    "unknown@unknown",
                ),
                name: None,
            },
            sender: None,
        },
[...]
}
--- message structure end ---
hello: barrr
Date: 1 Jan 1970 00:00:00 +0000
From: unknown@unknown
MIME-Version: 1.0

hello??
```

## `trace`: tracing parser recovery strategies on email collections

The `trace` tool runs the parser on a collection of emails, recording "recovery"
trace events. A recovery event is emitted when the parser followed a best-effort
strategy after encountering non-compliant input. The tool must be compiled by
enabling at least one of the `tracing-recover` or `tracing-unsupported`
features: `tracing-recover` events signal when the parser recognized a known
form of non-compliant input and was able to interpret it; `tracing-unsupported`
eveents signal when the parser encountered an unknown form of non-compliant
input and discarded it.

Example invocation:
``` sh
cargo run --features=tracing-recover,tracing-unsupported --example trace -- <emails>... > trace.json
```

where `<emails>` can be:
- a directory containing individual email files (subdirectories are supported)
- a `.mbox` file (see the [mbox format](https://en.wikipedia.org/wiki/Mbox))
- a `.zip` file containing individual email files
- a `.tar` file containing individual email files
- a single email file

The tool writes on its standard output the trace of recovery events, as json records (on per line). 
