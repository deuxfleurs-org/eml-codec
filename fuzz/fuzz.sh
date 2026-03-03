#!/usr/bin/env bash

cargo +nightly fuzz run "$1" -- -timeout=1 -max_len=1500

# cargo +nightly fuzz run "$1" -- -timeout=1 -max_len=1500 -ignore_crashes=1 -fork=1
