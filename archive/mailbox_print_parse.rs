#![no_main]

use eml_codec::print::{with_formatter, Print};
use eml_codec::imf::mailbox::{mailbox, MailboxRef};
use eml_codec::fuzz_eq::FuzzEq;
use libfuzzer_sys::fuzz_target;
use libfuzzer_sys::arbitrary;
use libfuzzer_sys::arbitrary::Arbitrary;

fuzz_target!(|msg: MailboxRef<'_>| {
    // eprintln!("msg:\n{:?}", msg);
    let printed = with_formatter(Some(0), |f| {
        msg.print(f)
    });
    // eprintln!("\nprinted:\n{}", String::from_utf8_lossy(&printed));
    let msg2 = match mailbox(&printed) {
        Err(nom::Err::Failure(nom::error::Error { input, .. })) => {
            // eprintln!("\nError:\n{}", String::from_utf8_lossy(input));
            panic!()
        },
        Err(e) => {
            // eprintln!("\nError:\n{:?}", e);
            panic!()
        },
        Ok((rest, msg2)) => {
            // eprintln!("\nRest:\n{}", String::from_utf8_lossy(&rest));
            assert_eq!(rest, b"");
            msg2
        },
    };
    // eprintln!("\nReparsed:\n{:?}\n\n", msg2);
    assert!(msg.fuzz_eq(&msg2));
});
