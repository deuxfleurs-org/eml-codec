#!/usr/bin/env bash

SCRIPT_DIR="$( dirname -- "$( readlink -f -- "$0"; )"; )"
cd "$SCRIPT_DIR/../.."
cargo run --features=tracing-discard --release --example trace -- "$SCRIPT_DIR/hamlet.zip" \
    | grep -P -v "References|In-Reply-To|MIME-Version|Received|Return-Path|Keywords|Content-ID|(to_part_encoding: invalid mechanism)" \
    > "$SCRIPT_DIR/trace.json"
