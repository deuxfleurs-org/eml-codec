#!/usr/bin/env bash

cargo run --features=tracing-unsupported --release --example trace -- ./enron_mail_20150507.tar > ./trace.json
