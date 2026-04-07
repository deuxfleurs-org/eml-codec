# `trace` tool

The `trace` example program runs the parser on a set of emails, recording
"recovery" trace events. It must be compiled by enabling at least one of the
`tracing-recover` or `tracing-discard` features.

Example invocation:
``` sh
cargo run --features=tracing-recover,tracing-discard --example trace -- <emails>... > trace.json
```

where `<emails>` can be:
- a directory containing individual email files (subdirectories are supported)
- a `.mbox` file (see the [mbox format](https://en.wikipedia.org/wiki/Mbox))
- a `.zip` file containing individual email files
- a single email file

The tool writes on its standard output the trace of recovery events, as json records (on per line). 
