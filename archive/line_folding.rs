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
    Crlf,
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
                if ascii::WS.contains(&txt[0]) {
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
            Cmd::Crlf => {
                at_bol = true;
                out.extend_from_slice(ascii::CRLF);
            }
        }
    }
    Some(out)
}

fn unfold_text(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut pos = 0;
    loop {
        match &data[pos..] {
            [] => return out,
            [ascii::CR, ascii::LF, w, ..] if ascii::WS.contains(w) => {
                pos += 2
            },
            [b, ..] => {
                out.push(*b);
                pos += 1
            }
        }
    }
}

fuzz_target!(|cmds: Vec<Cmd>| -> Corpus {
    let text = match text_of_cmds(&cmds) {
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
                Cmd::Crlf =>
                    fmt.write_crlf()
            }
        }
    });

    assert_eq!(text, unfold_text(&folded));

    Corpus::Keep
});
