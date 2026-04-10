#!/usr/bin/env bash

cargo run --features=tracing-unsupported --release --example trace -- ./jeeproject_yahoo_tranche1.zip > ./trace.json
