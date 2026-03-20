#!/bin/bash

if [ "$#" -lt 2 ]; then
    echo "Usage: $0 <fuzz target> <nb cores>"
    exit 1
fi

nice cargo +nightly fuzz run "$1" --release -s none -- -timeout=1 -max_len=2000 -jobs="$2"
