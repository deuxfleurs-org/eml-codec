use itertools::Itertools;
use nom::IResult;
use std::fmt;

use crate::header;
use crate::imf;
use crate::mime;
use crate::part::{self, AnyPart};
use crate::text::boundary::{boundary, Delimiter};

//--- Multipart
#[derive(PartialEq)]
pub struct Multipart<'a> {
    pub mime: mime::MIME<'a, mime::r#type::Multipart>,
    pub children: Vec<AnyPart<'a>>,
    pub preamble: &'a [u8],
    pub epilogue: &'a [u8],
}
impl<'a> fmt::Debug for Multipart<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("part::Multipart")
            .field("mime", &self.mime)
            .field("children", &self.children)
            .field("preamble", &String::from_utf8_lossy(self.preamble))
            .field("epilogue", &String::from_utf8_lossy(self.epilogue))
            .finish()
    }
}

pub fn multipart<'a>(
    m: mime::MIME<'a, mime::r#type::Multipart>,
) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], Multipart<'a>> {
    let m = m.clone();

    move |input| {
        // init
        let bound = m.ctype.boundary.as_bytes();
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
            let (input, naive_mime) = match header::header_kv(input) {
                Ok((input_eom, fields)) => {
                    // XXX: simplify this and part::field::split_and_build by moving this
                    // logic into a FromIterator<header::Field> -> NaiveMIME?
                    let (mime_headers, uninterp_headers): (Vec<_>, Vec<_>) = fields
                        .iter()
                        .map(|hdr| mime::field::Content::try_from(hdr).map_err(|_| hdr.clone()))
                        .partition_result();
                    let mut mime: mime::NaiveMIME = mime_headers.into_iter().collect();
                    mime.fields.uninterp_headers = uninterp_headers;

                    (input_eom, mime)
                }
                Err(_) => (input, mime::NaiveMIME::default()),
            };

            // interpret mime according to context
            let mime = match m.ctype.subtype {
                mime::r#type::MultipartSubtype::Digest => {
                    naive_mime.to_interpreted(mime::DefaultType::Digest).into()
                }
                _ => naive_mime.to_interpreted(mime::DefaultType::Generic).into(),
            };

            // parse raw part
            let (input, rpart) = part::part_raw(bound)(input)?;

            // parse mime body
            // -- we do not keep the input as we are using the
            // part_raw function as our cursor here.
            let (_, part) = part::anypart(mime)(rpart)?;
            mparts.push(part);

            input_loop = input;
        }
    }
}

//--- Message

#[derive(PartialEq)]
pub struct Message<'a> {
    pub mime: mime::MIME<'a, mime::r#type::DeductibleMessage>,
    pub imf: imf::Imf<'a>,
    pub child: Box<AnyPart<'a>>,
}
impl<'a> fmt::Debug for Message<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("part::Message")
            .field("mime", &self.mime)
            .field("imf", &self.imf)
            .field("child", &self.child)
            .finish()
    }
}

pub fn message<'a>(
    m: mime::MIME<'a, mime::r#type::DeductibleMessage>,
) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], Message<'a>> {
    move |input: &[u8]| {
        // parse header fields
        let (input, headers) = header::header_kv(input)?;

        //---------------
        // aggregate header fields
        let (naive_mime, imf) = part::field::split_and_build(&headers);

        // interpret headers to choose the child mime type
        let in_mime = naive_mime.to_interpreted(mime::DefaultType::Generic).into();
        //---------------

        // parse a part following this mime specification
        let (input, part) = part::anypart(in_mime)(input)?;

        Ok((
            input,
            Message {
                // XXX is feels weird that `mime` refers to the headers passed from
                // above while headers of the part are split into imf and child part
                mime: m.clone(),
                imf,
                child: Box::new(part),
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::part::discrete::Text;
    use crate::part::AnyPart;
    use crate::text::encoding::{Base64Word, EncodedWord, QuotedChunk, QuotedWord};
    use crate::text::misc_token::{Phrase, UnstrToken, Unstructured, Word};
    use chrono::{FixedOffset, TimeZone};

    #[test]
    fn test_multipart() {
        let base_mime = mime::MIME {
            ctype: mime::r#type::Multipart {
                subtype: mime::r#type::MultipartSubtype::Alternative,
                boundary: "simple boundary".to_string(),
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
                    preamble,
                    epilogue,
                    children: vec![
                        AnyPart::Txt(Text {
                            mime: mime::MIME {
                                ctype: mime::r#type::Deductible::Inferred(mime::r#type::Text {
                                    subtype: mime::r#type::TextSubtype::Plain,
                                    charset: mime::r#type::Deductible::Inferred(mime::charset::EmailCharset::US_ASCII),
                                }),
                                fields: mime::CommonMIME::default(),
                            },
                            body: &b"This is implicitly typed plain US-ASCII text.\nIt does NOT end with a linebreak."[..],
                        }),
                        AnyPart::Txt(Text {
                            mime: mime::MIME {
                                ctype: mime::r#type::Deductible::Explicit(mime::r#type::Text {
                                    subtype: mime::r#type::TextSubtype::Plain,
                                    charset: mime::r#type::Deductible::Explicit(mime::charset::EmailCharset::US_ASCII),
                                }),
                                fields: mime::CommonMIME::default(),
                            },
                            body: &b"This is explicitly typed plain US-ASCII text.\nIt DOES end with a linebreak.\n"[..],
                        }),
                    ],
                },
            ))
        );
    }

    #[test]
    fn test_message() {
        let fullmail: &[u8] = r#"Date: Sat, 8 Jul 2023 07:14:29 +0200
From: Grrrnd Zero <grrrndzero@example.org>
To: John Doe <jdoe@machine.example>
CC: =?ISO-8859-1?Q?Andr=E9?= Pirard <PIRARD@vm1.ulg.ac.be>
Subject: =?ISO-8859-1?B?SWYgeW91IGNhbiByZWFkIHRoaXMgeW8=?=
    =?ISO-8859-2?B?dSB1bmRlcnN0YW5kIHRoZSBleGFtcGxlLg==?=
X-Unknown: something something
Bad entry
  on multiple lines
Message-ID: <NTAxNzA2AC47634Y366BAMTY4ODc5MzQyODY0ODY5@www.grrrndzero.org>
MIME-Version: 1.0
Content-Type: multipart/alternative;
 boundary="b1_e376dc71bafc953c0b0fdeb9983a9956"
Content-Transfer-Encoding: 7bit

This is a multi-part message in MIME format.

--b1_e376dc71bafc953c0b0fdeb9983a9956
Content-Type: text/plain; charset=utf-8
Content-Transfer-Encoding: quoted-printable

GZ
OoOoO
oOoOoOoOo
oOoOoOoOoOoOoOoOo
oOoOoOoOoOoOoOoOoOoOoOo
oOoOoOoOoOoOoOoOoOoOoOoOoOoOo
OoOoOoOoOoOoOoOoOoOoOoOoOoOoOoOoO

--b1_e376dc71bafc953c0b0fdeb9983a9956
Content-Type: text/html; charset=us-ascii

<div style="text-align: center;"><strong>GZ</strong><br />
OoOoO<br />
oOoOoOoOo<br />
oOoOoOoOoOoOoOoOo<br />
oOoOoOoOoOoOoOoOoOoOoOo<br />
oOoOoOoOoOoOoOoOoOoOoOoOoOoOo<br />
OoOoOoOoOoOoOoOoOoOoOoOoOoOoOoOoO<br />
</div>

--b1_e376dc71bafc953c0b0fdeb9983a9956--
"#
        .as_bytes();

        let preamble = b"This is a multi-part message in MIME format.
";

        let base_mime = mime::MIME::<mime::r#type::DeductibleMessage>::default();
        assert_eq!(
            message(base_mime.clone())(fullmail),
            Ok((
                &[][..],
                Message {
                    mime: base_mime,
                    imf: imf::Imf {
                        date: Some(FixedOffset::east_opt(2 * 3600)
                            .unwrap()
                            .with_ymd_and_hms(2023, 07, 8, 7, 14, 29)
                            .unwrap()),
                        from: vec![
                            imf::mailbox::MailboxRef {
                                name: Some(Phrase(vec![Word::Atom(&b"Grrrnd"[..]), Word::Atom(&b"Zero"[..])])),
                                addrspec: imf::mailbox::AddrSpec {
                                    local_part: imf::mailbox::LocalPart(vec![
                                        imf::mailbox::LocalPartToken::Word(Word::Atom(&b"grrrndzero"[..]))
                                    ]),
                                    domain: imf::mailbox::Domain::Atoms(vec![&b"example"[..], &b"org"[..]]),
                                }
                            },
                        ],

                        to: vec![imf::address::AddressRef::Single(imf::mailbox::MailboxRef {
                                name: Some(Phrase(vec![Word::Atom(&b"John"[..]), Word::Atom(&b"Doe"[..])])),
                                addrspec: imf::mailbox::AddrSpec {
                                    local_part: imf::mailbox::LocalPart(vec![
                                        imf::mailbox::LocalPartToken::Word(Word::Atom(&b"jdoe"[..]))
                                    ]),
                                    domain: imf::mailbox::Domain::Atoms(vec![&b"machine"[..], &b"example"[..]]),
                                }
                         })],

                        cc: vec![imf::address::AddressRef::Single(imf::mailbox::MailboxRef {
                            name: Some(Phrase(vec![
                                Word::Encoded(EncodedWord::Quoted(QuotedWord {
                                    enc: encoding_rs::WINDOWS_1252,
                                    chunks: vec![
                                        QuotedChunk::Safe(&b"Andr"[..]),
                                        QuotedChunk::Encoded(vec![0xE9]),
                                    ],
                                })),
                                Word::Atom(&b"Pirard"[..])
                            ])),
                            addrspec: imf::mailbox::AddrSpec {
                                local_part: imf::mailbox::LocalPart(vec![
                                    imf::mailbox::LocalPartToken::Word(Word::Atom(&b"PIRARD"[..]))
                                ]),
                                domain: imf::mailbox::Domain::Atoms(vec![
                                    &b"vm1"[..], &b"ulg"[..], &b"ac"[..], &b"be"[..],
                                ]),
                            }
                        })],

                        subject: Some(Unstructured(vec![
                            UnstrToken::Encoded(EncodedWord::Base64(Base64Word{
                                enc: encoding_rs::WINDOWS_1252,
                                content: &b"SWYgeW91IGNhbiByZWFkIHRoaXMgeW8"[..],
                            })),
                            UnstrToken::Encoded(EncodedWord::Base64(Base64Word{
                                enc: encoding_rs::ISO_8859_2,
                                content: &b"dSB1bmRlcnN0YW5kIHRoZSBleGFtcGxlLg"[..],
                            })),
                        ])),
                        msg_id: Some(imf::identification::MessageID {
                            left: &b"NTAxNzA2AC47634Y366BAMTY4ODc5MzQyODY0ODY5"[..],
                            right: &b"www.grrrndzero.org"[..],
                        }),
                        mime_version: Some(imf::mime::Version { major: 1, minor: 0}),
                        ..imf::Imf::default()
                    },
                    child: Box::new(AnyPart::Mult(Multipart {
                        mime: mime::MIME {
                            ctype: mime::r#type::Multipart {
                                subtype: mime::r#type::MultipartSubtype::Alternative,
                                boundary: "b1_e376dc71bafc953c0b0fdeb9983a9956".to_string(),
                            },
                            fields: mime::CommonMIME {
                                uninterp_headers: vec![
                                    header::Field::Good(header::Kv2(&b"X-Unknown"[..], &b"something something"[..])),
                                    header::Field::Bad(&b"Bad entry\n  on multiple lines\n"[..]),
                                ],
                                ..mime::CommonMIME::default()
                            },
                        },
                        preamble,
                        epilogue: &[],
                        children: vec![
                            AnyPart::Txt(Text {
                                mime: mime::MIME {
                                    ctype: mime::r#type::Deductible::Explicit(mime::r#type::Text {
                                        subtype: mime::r#type::TextSubtype::Plain,
                                        charset: mime::r#type::Deductible::Explicit(mime::charset::EmailCharset::UTF_8),
                                    }),
                                    fields: mime::CommonMIME {
                                        transfer_encoding: mime::mechanism::Mechanism::QuotedPrintable,
                                        ..mime::CommonMIME::default()
                                    }
                                },
                                body: &b"GZ\nOoOoO\noOoOoOoOo\noOoOoOoOoOoOoOoOo\noOoOoOoOoOoOoOoOoOoOoOo\noOoOoOoOoOoOoOoOoOoOoOoOoOoOo\nOoOoOoOoOoOoOoOoOoOoOoOoOoOoOoOoO\n"[..],
                            }),
                            AnyPart::Txt(Text {
                                mime: mime::MIME {
                                    ctype: mime::r#type::Deductible::Explicit(mime::r#type::Text {
                                        subtype: mime::r#type::TextSubtype::Html,
                                        charset: mime::r#type::Deductible::Explicit(mime::charset::EmailCharset::US_ASCII),
                                    }),

                                    fields: mime::CommonMIME::default(),
                                },
                                body: &br#"<div style="text-align: center;"><strong>GZ</strong><br />
OoOoO<br />
oOoOoOoOo<br />
oOoOoOoOoOoOoOoOo<br />
oOoOoOoOoOoOoOoOoOoOoOo<br />
oOoOoOoOoOoOoOoOoOoOoOoOoOoOo<br />
OoOoOoOoOoOoOoOoOoOoOoOoOoOoOoOoO<br />
</div>
"#[..],
                            }),
                        ],
                    })),
                },
            ))
        );
    }
}
