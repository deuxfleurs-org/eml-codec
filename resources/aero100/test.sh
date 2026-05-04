#!/usr/bin/env bash

cargo run --features=tracing-unsupported --release --example trace -- ./aero100.mbox > ./trace.json
