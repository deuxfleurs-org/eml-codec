#[cfg(feature = "arbitrary")]
use arbitrary::Arbitrary;
use bounded_static::ToStatic;
use std::borrow::Cow;
use std::fmt;
#[cfg(feature = "tracing")]
use tracing::{Level, span};

#[cfg(feature = "arbitrary")]
use crate::fuzz_eq::FuzzEq;
use crate::i18n::ContainsUtf8;
use crate::header;
use crate::mime;
use crate::part::{self, AnyPart, field::NaiveEntityFields};
use crate::raw_input::RawInput;
use crate::text::boundary::{boundary, Delimiter};

//--- Multipart
#[derive(Clone, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub struct Multipart<'a> {
    pub mime: mime::MIME<'a, mime::r#type::Multipart<'a>>,
    pub children: Vec<AnyPart<'a>>,
    #[cfg_attr(feature = "arbitrary", fuzz_eq(ignore))]
    pub preamble: Cow<'a, [u8]>,
    #[cfg_attr(feature = "arbitrary", fuzz_eq(ignore))]
    pub epilogue: Cow<'a, [u8]>,
    pub raw_body: RawInput<'a>,
}
impl<'a> fmt::Debug for Multipart<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("part::Multipart")
            .field("mime", &self.mime)
            .field("children", &self.children)
            .field("preamble", &String::from_utf8_lossy(&self.preamble))
            .field("epilogue", &String::from_utf8_lossy(&self.epilogue))
            .field("raw_body", &self.raw_body)
            .finish()
    }
}
#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for Multipart<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Multipart {
            mime: u.arbitrary()?,
            children: u.arbitrary()?,
            preamble: b"".into(),
            epilogue: b"".into(),
            raw_body: RawInput::none(),
        })
    }
}

// REQUIRES: `m.ctype.boundary` is `Some(_)`. This is guaranteed by
// the parser for `mime::MIME<_, Multipart>`.
pub fn multipart<'a>(
    m: mime::MIME<'a, mime::r#type::Multipart<'a>>,
) -> impl Fn(&'a [u8]) -> (&'a [u8], Multipart<'a>) {
    let m = m.clone();

    move |input| {
        #[cfg(feature = "tracing")]
        let _span = span!(Level::DEBUG, "part::composite::multipart", ?m).entered();

        let full_input = input;

        // init
        // NOTE: the `.unwrap()` cannot fail as long as `m` is produced by
        // the parser, which always specifies a `boundary` (the boundary
        // used by the input).
        let bound = m.ctype.boundary.as_ref().unwrap().as_bytes();
        let part_raw = part_raw(bound);
        let mut mparts: Vec<AnyPart> = vec![];

        // preamble
        let (mut input_loop, preamble) = part_raw(input);

        loop {
            let input = match boundary(bound)(input_loop) {
                Err(_) => {
                    // We encountered a malformed boundary, stop parsing.
                    let raw_body = &full_input[0..full_input.len() - input_loop.len()];
                    return (
                        input_loop,
                        Multipart {
                            mime: m.clone(),
                            children: mparts,
                            preamble: preamble.into(),
                            epilogue: [][..].into(),
                            raw_body: raw_body.into(),
                        },
                    )
                }
                Ok((inp, Delimiter::Last)) => {
                    return (
                        &[],
                        Multipart {
                            mime: m.clone(),
                            children: mparts,
                            preamble: preamble.into(),
                            epilogue: inp.into(),
                            raw_body: full_input.into(),
                        },
                    )
                }
                Ok((inp, Delimiter::Next)) => inp,
            };

            // parse mime headers, otherwise pick default mime
            let (input_body, fields_raw) = header::header_kv(input);
            let NaiveEntityFields { entries, mime } =
                fields_raw.into_iter().collect::<NaiveEntityFields>();

            // interpret mime according to context
            let mime = match m.ctype.subtype {
                mime::r#type::MultipartSubtype::Digest =>
                    mime.to_interpreted(mime::DefaultType::Digest).into(),
                _ =>
                    mime.to_interpreted(mime::DefaultType::Generic).into(),
            };

            // parse raw part for the body
            let (input_next, rpart) = part_raw(input_body);

            // parse mime body
            // XXX this can be an (indirect) recursive call;
            // -> risk of stack overflow
            let mime_body = part::part_body(mime)(rpart);
            mparts.push(AnyPart {
                entries,
                mime_body,
                raw: input[0..input.len() - input_next.len()].into(),
                raw_headers: input[0..input.len() - input_body.len()].into(),
            });

            input_loop = input_next;
        }
    }
}

// Recognizes bytes for the next part, until the next boundary or the end of the input.
fn part_raw<'a, 'b>(bound: &[u8]) -> impl Fn(&'a [u8]) -> (&'a [u8], &'a [u8]) + 'b {
    use memchr::memmem::Finder;
    // This low-level implementation (which basically just calls `memmem`) is faster
    // than trying to express this using parser combinators.

    // search for "--{bound}"
    let mut needle = b"--".to_vec();
    needle.extend(bound.iter());
    let finder = Finder::new(&needle).into_owned();

    move |input| {
        for i in finder.find_iter(input) {
            // a boundary can be at the beginning of the input
            if i == 0 {
                return (&input, &[])
            }

            // or it can be after a newline
            if i.checked_sub(1).is_some_and(|j| input[j] == b'\n') {
                // best-effort: recognize both \n and \r\n before the boundary
                let i = i.checked_sub(2).filter(|j| input[*j] == b'\r').unwrap_or(i-1);
                return (&input[i..], &input[0..i])
            }
        }
        // no matching boundary found; return the entire input
        (&[], input)
    }
}

//--- Message

// Invariant: if message headers use non-ascii UTF-8, message subtype RFC822
// must not be used and subtype Global must be used instead.
#[derive(Clone, Debug, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub struct Message<'a> {
    pub mime: mime::MIME<'a, mime::r#type::Message<'a>>,

    // NOTE: RFC2046 does not define the contents of an encapsulated message to
    // be a "part" (instead parts are the children of a multipart entity).
    // Intuitively, the contents of an encapsulated message should be a toplevel
    // message (`message::Message`) in most cases.
    //
    // However, RFC2046 specifies that an encapsulated message "isn't restricted
    // to material in strict conformance to RFC822" and that it "could well be a
    // News article or a MIME message".
    //
    // We thus decide to parse the contents as a generic MIME entity using
    // AnyPart. A downside of this approach is that we parse non-MIME headers as
    // "unstructured", even though it could make more sense to keep them as raw
    // bytes so that they could easily be parsed further. In any case,
    // unstructured headers can always be printed back to bytes without any loss
    // of information, so further parsing is possible, just not zero-copy
    // anymore.
    pub child: Box<AnyPart<'a>>,
    pub raw_body: RawInput<'a>,
}

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for Message<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let mut mime: mime::MIME<'a, mime::r#type::Message<'a>> = u.arbitrary()?;
        let child: Box<AnyPart<'a>> = u.arbitrary()?;
        // TODO: clarify whether we should take the body into account as well, and
        // not just the headers (for later when we start interpreting bodies?)
        if matches!(mime.ctype.subtype, mime::r#type::MessageSubtype::RFC822) &&
            child.contains_utf8_headers()
        {
            mime.ctype.subtype = mime::r#type::MessageSubtype::Global
        }
        Ok(Message { mime, child, raw_body: RawInput::none() })
    }
}

/// Parse an embedded message.
///
/// This function always consumes its entire input.
pub fn message<'a>(
    m: mime::MIME<'a, mime::r#type::Message<'a>>,
) -> impl Fn(&'a [u8]) -> Message<'a> {
    move |input: &[u8]| {
        #[cfg(feature = "tracing")]
        let _span = span!(Level::DEBUG, "part::composite::message", ?m).entered();

        // parse header fields
        let (input_body, headers) = header::header_kv(input);
        // detect UTF-8 use in headers
        let has_utf8 = headers.iter().any(|f| f.contains_utf8());
        let fields: NaiveEntityFields = headers.into_iter().collect();

        let mut msg_mime = m.clone();
        // If the headers contain non-ascii UTF8 and if this is a
        // message/RFC822, promote the message outer MIME to message/global
        if has_utf8 &&
            matches!(msg_mime.ctype.subtype, mime::r#type::MessageSubtype::RFC822)
        {
            msg_mime.ctype.subtype = mime::r#type::MessageSubtype::Global;
        }

        // interpret headers to choose the child mime type
        let in_mime = fields.mime.to_interpreted(mime::DefaultType::Generic).into();
        //---------------

        // parse the body following this mime specification
        let mime_body = part::part_body(in_mime)(input_body);

        Message {
            mime: msg_mime,
            child: Box::new(AnyPart {
                entries: fields.entries,
                mime_body,
                raw: input.into(),
                raw_headers: input[0..input.len() - input_body.len()].into(),
            }),
            raw_body: input_body.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mime::field::Entry;
    use crate::part::discrete::Text;
    use crate::part::{AnyPart, MimeBody};
    use crate::part::field::EntityEntry;
    use crate::text::charset::EmailCharset;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_preamble() {
        assert_eq!(
            part_raw(b"hello")(
                b"blip
bloup

blip
bloup--
--bim
--bim--

--hello
Field: Body
"
            ),
            (
                &b"\n--hello\nField: Body\n"[..],
                &b"blip\nbloup\n\nblip\nbloup--\n--bim\n--bim--\n"[..],
            )
        );
    }

    #[test]
    fn test_part_raw() {
        assert_eq!(
            part_raw(b"simple boundary")(b"Content-type: text/plain; charset=us-ascii

This is explicitly typed plain US-ASCII text.
It DOES end with a linebreak.

--simple boundary--
"),
            (
                &b"\n--simple boundary--\n"[..],
                &b"Content-type: text/plain; charset=us-ascii\n\nThis is explicitly typed plain US-ASCII text.\nIt DOES end with a linebreak.\n"[..],
            )
        );
    }

    #[test]
    fn test_multipart() {
        let base_mime = mime::MIME {
            ctype: mime::r#type::Multipart {
                subtype: mime::r#type::MultipartSubtype::Alternative,
                boundary: Some("simple boundary".to_string()),
                other_params: vec![],
            },
            fields: mime::CommonMIME::default(),
        };

        let input = b"This is the preamble.  It is to be ignored, though it
is a handy place for composition agents to include an
explanatory note to non-MIME conformant readers.

--simple boundary

This is implicitly typed plain US-ASCII text.
It does NOT end with a linebreak.
--simple boundary
Content-type: text/plain; charset=us-ascii

This is explicitly typed plain US-ASCII text.
It DOES end with a linebreak.

--simple boundary--

This is the epilogue. It is also to be ignored.
";

        let preamble = b"This is the preamble.  It is to be ignored, though it
is a handy place for composition agents to include an
explanatory note to non-MIME conformant readers.
";

        let epilogue = b"
This is the epilogue. It is also to be ignored.
";

        assert_eq!(
            multipart(base_mime.clone())(input),
            (&b""[..],
             Multipart {
                 mime: base_mime,
                 preamble: preamble.into(),
                 epilogue: epilogue.into(),
                 children: vec![
                     AnyPart {
                         entries: vec![],
                         mime_body: MimeBody::Txt(Text {
                             mime: mime::MIME {
                                 ctype: mime::r#type::Text::default(),
                                 fields: mime::CommonMIME::default(),
                             },
                             body: b"This is implicitly typed plain US-ASCII text.\nIt does NOT end with a linebreak.".into(),
                             raw_body: RawInput::between(input, b"This is implicitly", b"NOT end with a linebreak."),
                         }),
                         raw: RawInput::between(input, b"\nThis is implicitly", b"NOT end with a linebreak."),
                         raw_headers: b"\n".into(),
                     },
                     AnyPart {
                         entries: vec![EntityEntry::MIME { e: Entry::Type, raw_body: b" text/plain; charset=us-ascii".into() }],
                         mime_body: MimeBody::Txt(Text {
                             mime: mime::MIME {
                                 ctype: mime::r#type::Text {
                                     subtype: mime::r#type::TextSubtype::Plain,
                                     charset: EmailCharset::US_ASCII,
                                     other_params: vec![],
                                 },
                                 fields: mime::CommonMIME::default(),
                             },
                             body: b"This is explicitly typed plain US-ASCII text.\nIt DOES end with a linebreak.\n".into(),
                             raw_body: RawInput::between(input, b"This is explicitly", b"DOES end with a linebreak.\n"),
                         }),
                         raw: RawInput::between(input, b"Content-type", b"DOES end with a linebreak.\n"),
                         raw_headers: b"Content-type: text/plain; charset=us-ascii\n\n".into(),
                     },
                 ],
                 raw_body: input.into(),
             },
            )
        );
    }

    // The terminator of a multipart entity can be missing.
    // This should be properly handled even for nested multiparts
    // (RFC2046 specifies this in sec 5.1.2).
    #[test]
    fn test_nested_multipart_inner_broken() {
        let base_mime = mime::MIME {
            ctype: mime::r#type::Multipart {
                subtype: mime::r#type::MultipartSubtype::Mixed,
                boundary: Some("outer boundary".to_string()),
                other_params: vec![],
            },
            fields: mime::CommonMIME::default(),
        };

        let input = b"
--outer boundary
Content-Type: multipart/mixed; boundary=\"inner boundary\"

--inner boundary

This is the inner part; it misses its terminator
--outer boundary

This is implicitly typed plain US-ASCII text.
--outer boundary--";

        assert_eq!(
            multipart(base_mime.clone())(input),
            (&b""[..],
             Multipart {
                 mime: base_mime,
                 preamble: b"".into(),
                 epilogue: b"".into(),
                 children: vec![
                     AnyPart {
                         entries: vec![
                             EntityEntry::MIME {
                                 e: Entry::Type,
                                 raw_body: b" multipart/mixed; boundary=\"inner boundary\"".into(),
                             },
                         ],
                         mime_body: MimeBody::Mult(Multipart {
                             mime: mime::MIME {
                                 ctype: mime::r#type::Multipart {
                                     subtype: mime::r#type::MultipartSubtype::Mixed,
                                     boundary: Some("inner boundary".to_string()),
                                     other_params: vec![],
                                 },
                                 fields: mime::CommonMIME::default(),
                             },
                             preamble: b"".into(),
                             epilogue: b"".into(),
                             children: vec![
                                 AnyPart {
                                     entries: vec![],
                                     mime_body: MimeBody::Txt(Text {
                                         mime: mime::MIME {
                                             ctype: mime::r#type::Text::default(),
                                             fields: mime::CommonMIME::default(),
                                         },
                                         body: b"This is the inner part; it misses its terminator".into(),
                                         raw_body: RawInput::between(input, b"This is the inner", b"terminator"),
                                     }),
                                     raw: RawInput::between(input, b"\nThis is the inner", b"terminator"),
                                     raw_headers: b"\n".into(),
                                 },
                             ],
                             raw_body: RawInput::between(input, b"--inner boundary\n\nThis is the inner", b"terminator"),
                         }),
                         raw: RawInput::between(input, b"Content-Type", b"terminator"),
                         raw_headers: b"Content-Type: multipart/mixed; boundary=\"inner boundary\"\n\n".into(),
                     },
                     AnyPart {
                         entries: vec![],
                         mime_body: MimeBody::Txt(Text {
                             mime: mime::MIME {
                                 ctype: mime::r#type::Text::default(),
                                 fields: mime::CommonMIME::default(),
                             },
                             body: b"This is implicitly typed plain US-ASCII text.".into(),
                             raw_body: b"This is implicitly typed plain US-ASCII text.".into(),
                         }),
                         raw: b"\nThis is implicitly typed plain US-ASCII text.".into(),
                         raw_headers: b"\n".into(),
                     },
                 ],
                 raw_body: input.into(),
             },
            )
        );
    }

    // Parsing stops on a broken boundary that starts with the correct boundary
    // but is followed by a suffix containing junk
    // FIXME: the RFC requires that we handle whitespace characters as a suffix,
    // but this is not done currently.
    #[test]
    fn test_broken_boundary() {
        let base_mime = mime::MIME {
            ctype: mime::r#type::Multipart {
                subtype: mime::r#type::MultipartSubtype::Mixed,
                boundary: Some("boundary".to_string()),
                other_params: vec![],
            },
            fields: mime::CommonMIME::default(),
        };

        let input = b"
--boundary

Part text
--boundary+++out of cheese

leftovers";

        assert_eq!(
            multipart(base_mime.clone())(input),
            (&b"\n--boundary+++out of cheese\n\nleftovers"[..],
             Multipart {
                 mime: base_mime,
                 preamble: b"".into(),
                 epilogue: b"".into(),
                 children: vec![
                     AnyPart {
                         entries: vec![],
                         mime_body: MimeBody::Txt(Text {
                             mime: mime::MIME {
                                 ctype: mime::r#type::Text::default(),
                                 fields: mime::CommonMIME::default(),
                             },
                             body: b"Part text".into(),
                             raw_body: b"Part text".into(),
                         }),
                         raw: b"\nPart text".into(),
                         raw_headers: b"\n".into(),
                     },
                 ],
                 raw_body: b"\n--boundary\n\nPart text".into(),
             },
            )
        );
    }

    #[test]
    fn test_multipart_cr() {
        let base_mime = mime::MIME {
            ctype: mime::r#type::Multipart {
                subtype: mime::r#type::MultipartSubtype::Alternative,
                boundary: Some("boundary".to_string()),
                other_params: vec![],
            },
            fields: mime::CommonMIME::default(),
        };

        let input = b"--boundary

\r\r
--boundary--
";

        assert_eq!(
            multipart(base_mime.clone())(input),
            (&b""[..],
             Multipart {
                 mime: base_mime,
                 preamble: b"".into(),
                 epilogue: b"".into(),
                 children: vec![
                     AnyPart {
                         entries: vec![],
                         mime_body: MimeBody::Txt(Text {
                             mime: mime::MIME {
                                 ctype: mime::r#type::Text::default(),
                                 fields: mime::CommonMIME::default(),
                             },
                             body: b"\r".into(),
                             raw_body: b"\r".into(),
                         }),
                         raw: b"\n\r".into(),
                         raw_headers: b"\n".into(),
                     },
                 ],
                 raw_body: input.into(),
             },
            )
        );
    }
}
