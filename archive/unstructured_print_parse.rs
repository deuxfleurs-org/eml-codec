#![no_main]

use eml_codec::print::{with_formatter, Fmt, Formatter};
use libfuzzer_sys::{fuzz_target, Corpus};
use libfuzzer_sys::arbitrary;
use libfuzzer_sys::arbitrary::Arbitrary;

fuzz_target!(|cmds: Vec<Cmd>| -> Corpus {

    Corpus::Keep
});
