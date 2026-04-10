#!/usr/bin/env bash

cargo run --features=tracing-unsupported --release --example trace -- ./camllist.zip > ./trace.json
