#!/usr/bin/env bash

SCRIPT_DIR="$( dirname -- "$( readlink -f -- "$0"; )"; )"
cd "$SCRIPT_DIR/../.."
cargo run --features=tracing-discard --release --example trace -- "$SCRIPT_DIR/enron_mail_20150507.tar" > "$SCRIPT_DIR/trace.json"
