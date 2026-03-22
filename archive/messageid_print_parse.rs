#![no_main]

use eml_codec::print::{with_formatter, Print};
use eml_codec::imf::identification::{msg_id, MessageID};
use eml_codec::fuzz_eq::FuzzEq;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|msg: MessageID<'_>| {
    eprintln!("{:#?}", msg);
    let printed = with_formatter(Some(0), |f| {
        msg.print(f)
    });
    eprintln!("printed: {}", &String::from_utf8_lossy(&printed));
    eprintln!("printed: {:?}", &printed);
    let msg2 = match msg_id(&printed) {
        Err(nom::Err::Failure(nom::error::Error { input, .. })) => {
            eprintln!("\nError:\n{}", String::from_utf8_lossy(input));
            panic!()
        },
        Err(e) => {
            eprintln!("\nError:\n{:?}", e);
            panic!()
        },
        Ok((rest, msg2)) => {
            eprintln!("\nRest:\n{}", String::from_utf8_lossy(&rest));
            assert_eq!(rest, b"");
            msg2
        },
    };
    assert!(msg.fuzz_eq(&msg2));
});
