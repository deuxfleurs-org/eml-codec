#![no_main]

use eml_codec::print::{print_to_vec, FMT_DEFAULT};
use eml_codec::message::Message;
use eml_codec::fuzz_eq::FuzzEq;
use libfuzzer_sys::fuzz_target;
use pretty_assertions::Comparison;

fuzz_target!(|msg: Message<'_>| {
    let printed = print_to_vec(FMT_DEFAULT.with_seed(Some(0)), &msg);
    let msg2 = eml_codec::parse_message(&printed);
    if !msg.fuzz_eq(&msg2) {
        eprintln!("msg:\n{:#?}", msg);
        eprintln!("\n\nprinted:\n{}", String::from_utf8_lossy(&printed));
        eprintln!("\n\nReparsed:\n{:#?}", msg2);
        eprint!("{}", Comparison::new(&msg, &msg2));
        panic!()
    }
});
