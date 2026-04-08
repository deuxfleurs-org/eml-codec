#!/usr/bin/env bash

cargo run --features=tracing-discard --release --example trace -- ./hamlet.zip \
    | grep -P -v "References|In-Reply-To|MIME-Version|Received|Return-Path|Keywords|Content-ID|(to_part_encoding: invalid mechanism)" \
    > trace.json
