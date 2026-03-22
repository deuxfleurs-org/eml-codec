#![no_main]

use eml_codec::text::ascii;
use eml_codec::print::{with_formatter, Fmt, Formatter};
use libfuzzer_sys::{fuzz_target, Corpus};
use libfuzzer_sys::arbitrary;
use libfuzzer_sys::arbitrary::Arbitrary;

#[derive(Debug, Arbitrary)]
enum Cmd<'a> {
    Txt(&'a [u8]),
    Fws(&'a [u8]),
}

fn text_of_cmds(cmds: &[Cmd]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut at_bol = true;
    for cmd in cmds {
        match cmd {
            Cmd::Txt(txt) => {
                if txt.is_empty() {
                    return None
                }
                if txt.windows(2).find(|&s| s == ascii::CRLF).is_some() {
                    return None
                }
                if txt.iter().any(|b| ascii::WS.contains(b)) {
                    return None
                }
                // XXX oof
                if out.last().is_some_and(|b| *b == ascii::CR) && txt[0] == ascii::LF {
                    return None
                }

                at_bol = false;
                out.extend_from_slice(txt);
            },
            Cmd::Fws(fws) => {
                if at_bol {
                    return None
                }
                if fws.is_empty() {
                    return None
                }
                if !fws.iter().all(|&b| b == b' ' || b == b'\t') {
                    return None
                }
                out.extend_from_slice(fws)
            },
        }
    }
    Some(out)
}

fn check_no_whitespace_line(data: &[u8]) {
    let mut only_ws = true;
    let mut pos = 0;
    loop {
        match &data[pos..] {
            [] => {
                assert!(!only_ws);
                return
            }
            [ascii::CR, ascii::LF, ..] => {
                assert!(!only_ws);
                pos += 2;
            },
            [b, ..] => {
                only_ws = only_ws && ascii::WS.contains(b);
                pos += 1
            },
        }
    }
}

fn check_single_line(data: &[u8]) {
    assert!(data.windows(ascii::CRLF.len()).find(|s| *s == ascii::CRLF).is_none())
}

fuzz_target!(|cmds: Vec<Cmd>| -> Corpus {
    let _text = match text_of_cmds(&cmds) {
        Some(txt) => txt,
        None => return Corpus::Reject
    };

    let folded = with_formatter(Some(0), |fmt: &mut Fmt| {
        fmt.begin_line_folding();
        for cmd in &cmds {
            match cmd {
                Cmd::Txt(txt) => {
                    fmt.write_bytes(txt);
                },
                Cmd::Fws(fws) => {
                    fmt.write_fws_bytes(fws);
                },
            }
        }
    });

    if cmds.iter().all(|cmd| matches!(cmd, Cmd::Fws(_))) {
        check_single_line(&folded)
    } else {
        check_no_whitespace_line(&folded)
    }

    Corpus::Keep
});
