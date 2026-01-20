use bounded_static::ToStatic;
use nom::IResult;
use std::borrow::Cow;
use std::fmt;

use crate::header;
use crate::mime;
use crate::part::{self, AnyPart, field::EntityFields};
use crate::text::boundary::{boundary, Delimiter};

//--- Multipart
#[derive(Clone, PartialEq, ToStatic)]
pub struct Multipart<'a> {
    pub mime: mime::MIME<'a, mime::r#type::Multipart<'a>>,
    pub children: Vec<AnyPart<'a>>,
    pub preamble: Cow<'a, [u8]>,
    pub epilogue: Cow<'a, [u8]>,
}
impl<'a> fmt::Debug for Multipart<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("part::Multipart")
            .field("mime", &self.mime)
            .field("children", &self.children)
            .field("preamble", &String::from_utf8_lossy(&self.preamble))
            .field("epilogue", &String::from_utf8_lossy(&self.epilogue))
            .finish()
    }
}

// REQUIRES: `m.ctype.boundary` is `Some(_)`. This is guaranteed by
// the parser for `mime::MIME<_, Multipart>`.
pub fn multipart<'a>(
    m: mime::MIME<'a, mime::r#type::Multipart<'a>>,
) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], Multipart<'a>> {
    let m = m.clone();

    move |input| {
        // init
        // NOTE: the `.unwrap()` cannot fail as long as `m` is produced by
        // the parser, which always specifies a `boundary` (the boundary
        // used by the input).
        let bound = m.ctype.boundary.as_ref().unwrap();
        let mut mparts: Vec<AnyPart> = vec![];

        // preamble
        let (mut input_loop, preamble) = part::part_raw(bound)(input)?;

        loop {
            let input = match boundary(bound)(input_loop) {
                Err(_) => {
                    return Ok((
                        input_loop,
                        Multipart {
                            mime: m.clone(),
                            children: mparts,
                            preamble: preamble.into(),
                            epilogue: [][..].into(),
                        },
                    ))
                }
                Ok((inp, Delimiter::Last)) => {
                    return Ok((
                        inp,
                        Multipart {
                            mime: m.clone(),
                            children: mparts,
                            preamble: preamble.into(),
                            epilogue: inp.into(),
                        },
                    ))
                }
                Ok((inp, Delimiter::Next)) => inp,
            };

            // parse mime headers, otherwise pick default mime
            let (input, fields) = match header::header_kv(input) {
                Ok((input_eom, fields)) =>
                    (input_eom, fields.into_iter().collect::<EntityFields>()),
                Err(_) => (input, EntityFields::default()),
            };

            // interpret mime according to context
            let mime = match m.ctype.subtype {
                mime::r#type::MultipartSubtype::Digest => {
                    fields.mime.to_interpreted(mime::DefaultType::Digest).into()
                }
                _ => fields.mime.to_interpreted(mime::DefaultType::Generic).into(),
            };

            // parse raw part
            let (input, rpart) = part::part_raw(bound)(input)?;

            // parse mime body
            // -- we do not keep the input as we are using the
            // part_raw function as our cursor here.
            // XXX this can be an (indirect) recursive call;
            // -> risk of stack overflow
            let (_, mime_body) = part::part_body(mime)(rpart)?;
            mparts.push(AnyPart { fields: fields.all_fields, mime_body });

            input_loop = input;
        }
    }
}

//--- Message

#[derive(Clone, PartialEq, ToStatic)]
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
}
impl<'a> fmt::Debug for Message<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("part::Message")
            .field("mime", &self.mime)
            .field("child", &self.child)
            .finish()
    }
}

pub fn message<'a>(
    m: mime::MIME<'a, mime::r#type::Message<'a>>,
) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], Message<'a>> {
    move |input: &[u8]| {
        // parse header fields
        let (input, headers) = header::header_kv(input)?;
        let fields: EntityFields = headers.into_iter().collect();

        // interpret headers to choose the child mime type
        let in_mime = fields.mime.to_interpreted(mime::DefaultType::Generic).into();
        //---------------

        // parse the body following this mime specification
        let (input, mime_body) = part::part_body(in_mime)(input)?;

        Ok((
            input,
            Message {
                mime: m.clone(),
                child: Box::new(AnyPart { fields: fields.all_fields, mime_body }),
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mime::field::Entry;
    use crate::part::discrete::Text;
    use crate::part::{AnyPart, MimeBody};
    use crate::part::field::EntityField;
    use crate::utils::Deductible;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_multipart() {
        let base_mime = mime::MIME {
            ctype: mime::r#type::Multipart {
                subtype: mime::r#type::MultipartSubtype::Alternative,
                boundary: Some(b"simple boundary".to_vec()),
                params: vec![],
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
            Ok((&b"\nThis is the epilogue. It is also to be ignored.\n"[..],
                Multipart {
                    mime: base_mime,
                    preamble: preamble.into(),
                    epilogue: epilogue.into(),
                    children: vec![
                        AnyPart {
                            fields: vec![],
                            mime_body: MimeBody::Txt(Text {
                                mime: mime::MIME {
                                    ctype: Deductible::Inferred(mime::r#type::Text {
                                        subtype: mime::r#type::TextSubtype::Plain,
                                        charset: Deductible::Inferred(mime::charset::EmailCharset::US_ASCII),
                                        params: vec![],
                                    }),
                                    fields: mime::CommonMIME::default(),
                                },
                                body: b"This is implicitly typed plain US-ASCII text.\nIt does NOT end with a linebreak."[..].into(),
                            }),
                        },
                        AnyPart {
                            fields: vec![EntityField::MIME(Entry::Type)],
                            mime_body: MimeBody::Txt(Text {
                                mime: mime::MIME {
                                    ctype: Deductible::Explicit(mime::r#type::Text {
                                        subtype: mime::r#type::TextSubtype::Plain,
                                        charset: Deductible::Explicit(mime::charset::EmailCharset::US_ASCII),
                                        params: vec![],
                                    }),
                                    fields: mime::CommonMIME::default(),
                                },
                                body: b"This is explicitly typed plain US-ASCII text.\nIt DOES end with a linebreak.\n"[..].into(),
                            }),
                        },
                    ],
                },
            ))
        );
    }

    // The terminator of a multipart entity can be missing.
    // This should be properly handled even for nested multiparts
    // (RFC2046 specifies that this in sec 5.1.2).
    #[test]
    fn test_nested_multipart_inner_broken() {
        let base_mime = mime::MIME {
            ctype: mime::r#type::Multipart {
                subtype: mime::r#type::MultipartSubtype::Mixed,
                boundary: Some(b"outer boundary".to_vec()),
                params: vec![],
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
            Ok((&b""[..],
                Multipart {
                    mime: base_mime,
                    preamble: b"".into(),
                    epilogue: b"".into(),
                    children: vec![
                        AnyPart {
                            fields: vec![EntityField::MIME(Entry::Type)],
                            mime_body: MimeBody::Mult(Multipart {
                                mime: mime::MIME {
                                    ctype: mime::r#type::Multipart {
                                        subtype: mime::r#type::MultipartSubtype::Mixed,
                                        boundary: Some(b"inner boundary".to_vec()),
                                        params: vec![],
                                    },
                                    fields: mime::CommonMIME::default(),
                                },
                                preamble: b"".into(),
                                epilogue: b"".into(),
                                children: vec![
                                    AnyPart {
                                        fields: vec![],
                                        mime_body: MimeBody::Txt(Text {
                                            mime: mime::MIME {
                                                ctype: Deductible::Inferred(mime::r#type::Text {
                                                    subtype: mime::r#type::TextSubtype::Plain,
                                                    charset: Deductible::Inferred(mime::charset::EmailCharset::US_ASCII),
                                                    params: vec![],
                                                }),
                                                fields: mime::CommonMIME::default(),
                                            },
                                            body: b"This is the inner part; it misses its terminator"[..].into(),
                                        }),
                                    },
                                ],
                            }),
                        },
                        AnyPart {
                            fields: vec![],
                            mime_body: MimeBody::Txt(Text {
                                mime: mime::MIME {
                                    ctype: Deductible::Inferred(mime::r#type::Text {
                                        subtype: mime::r#type::TextSubtype::Plain,
                                        charset: Deductible::Inferred(mime::charset::EmailCharset::US_ASCII),
                                        params: vec![],
                                    }),
                                    fields: mime::CommonMIME::default(),
                                },
                                body: b"This is implicitly typed plain US-ASCII text."[..].into(),
                            }),
                        },
                    ],
                },
            ))
        );
    }
}
