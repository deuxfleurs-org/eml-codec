#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|bytes: &[u8]| {
    let _msg = eml_codec::parse_message(bytes);
});
