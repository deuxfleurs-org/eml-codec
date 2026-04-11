#!/usr/bin/env bash

cargo run --features=tracing-unsupported --release --example trace -- ./hamlet.zip \
    | grep -P -v "COPYING|README.md|References|In-Reply-To|MIME-Version|Received|Return-Path|Keywords|Content-ID|to_message_encoding: ignoring invalid mechanism|discarding segment in parameter list" \
    > trace.json
