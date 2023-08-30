use nom::IResult;

use crate::header;
use crate::imf;
use crate::mime;
use crate::part::{self, AnyPart};
use crate::text::boundary::{boundary, Delimiter};
use crate::pointers;

//--- Multipart
#[derive(Debug, PartialEq)]
pub struct Multipart<'a> {
    pub mime: mime::MIME<'a, mime::r#type::Multipart>,
    pub children: Vec<AnyPart<'a>>,
    pub raw_part_inner: &'a [u8],
    pub raw_part_outer: &'a [u8],
}
impl<'a> Multipart<'a> {
    pub fn preamble(&self) -> &'a [u8] {
        pointers::parsed(self.raw_part_outer, self.raw_part_inner)
    }
    pub fn epilogue(&self) -> &'a [u8] {
        pointers::rest(self.raw_part_outer, self.raw_part_inner)
    }
    pub fn preamble_and_body(&self) -> &'a [u8] {
        pointers::with_preamble(self.raw_part_outer, self.raw_part_inner)
    }
    pub fn body_and_epilogue(&self) -> &'a [u8] {
        pointers::with_epilogue(self.raw_part_outer, self.raw_part_inner)
    }
}

pub fn multipart<'a>(
    m: mime::MIME<'a, mime::r#type::Multipart>,
) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], Multipart<'a>> {
    let m = m.clone();

    move |input| {
        // init
        let outer_orig = input;
        let bound = m.interpreted_type.boundary.as_bytes();
        let mut mparts: Vec<AnyPart> = vec![];

        // skip preamble
        let (mut input_loop, _) = part::part_raw(bound)(input)?;
        let inner_orig = input_loop;

        loop {
            let input = match boundary(bound)(input_loop) {
                Err(_) => {
                    return Ok((
                        input_loop,
                        Multipart {
                            mime: m.clone(),
                            children: mparts,
                            raw_part_inner: pointers::parsed(inner_orig, input_loop),
                            raw_part_outer: pointers::parsed(outer_orig, input_loop),
                        },
                    ))
                }
                Ok((inp, Delimiter::Last)) => {
                    return Ok((
                        inp,
                        Multipart {
                            mime: m.clone(),
                            children: mparts,
                            raw_part_inner: pointers::parsed(inner_orig, inp),
                            raw_part_outer: pointers::parsed(outer_orig, &outer_orig[outer_orig.len()..]),
                        },
                    ))
                }
                Ok((inp, Delimiter::Next)) => inp,
            };

            // parse mime headers, otherwise pick default mime
            let (input, naive_mime) = match header::header_kv(input) {
                Ok((input_eom, fields)) => {
                    let raw_hdrs = pointers::parsed(input, input_eom);
                    let mime = fields
                        .iter()
                        .flat_map(mime::field::Content::try_from)
                        .into_iter()
                        .collect::<mime::NaiveMIME>();

                    let mime = mime
                        .with_fields(fields)
                        .with_raw(raw_hdrs);

                    (input_eom, mime)
                },
                Err(_) => (input, mime::NaiveMIME::default()),
            };

            // interpret mime according to context
            let mime = match m.interpreted_type.subtype {
                mime::r#type::MultipartSubtype::Digest => naive_mime.to_interpreted::<mime::WithDigestDefault>().into(),
                _ => naive_mime.to_interpreted::<mime::WithGenericDefault>().into(),
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

#[derive(Debug, PartialEq)]
pub struct Message<'a> {
    pub mime: mime::MIME<'a, mime::r#type::DeductibleMessage>,
    pub imf: imf::Imf<'a>,
    pub child: Box<AnyPart<'a>>,

    pub raw_part: &'a [u8],
    pub raw_headers: &'a [u8],
    pub raw_body: &'a [u8],
}

pub fn message<'a>(
    m: mime::MIME<'a, mime::r#type::DeductibleMessage>,
) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], Message<'a>> {
    move |input: &[u8]| {
        let orig = input;

        // parse header fields
        let (input, headers) = header::header_kv(input)?;

        // extract raw parts 1/2
        let raw_headers = pointers::parsed(orig, input);
        let body_orig = input;

        //---------------
        // aggregate header fields
        let (naive_mime, imf) = part::field::split_and_build(&headers);

        // interpret headers to choose a mime type
        let in_mime = naive_mime.with_fields(headers).with_raw(raw_headers).to_interpreted::<mime::WithGenericDefault>().into();
        //---------------

        // parse a part following this mime specification
        let (input, part) = part::anypart(in_mime)(input)?;

        // extract raw parts 2/2
        let raw_body = pointers::parsed(body_orig, input);
        let raw_part = pointers::parsed(orig, input);

        Ok((
            input,
            Message {
                mime: m.clone(),
                imf,
                raw_part, raw_headers, raw_body,
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
    use crate::text::misc_token::{Phrase, UnstrToken, Unstructured, Word, MIMEWord};
    use crate::text::quoted::QuotedString;
    use chrono::{FixedOffset, TimeZone};

    #[test]
    fn test_multipart() {
        let base_mime = mime::MIME {
            interpreted_type: mime::r#type::Multipart {
                subtype: mime::r#type::MultipartSubtype::Alternative,
                boundary: "simple boundary".to_string(),
            },
            fields: mime::NaiveMIME::default(),
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

        let inner = b"
--simple boundary

This is implicitly typed plain US-ASCII text.
It does NOT end with a linebreak.
--simple boundary
Content-type: text/plain; charset=us-ascii

This is explicitly typed plain US-ASCII text.
It DOES end with a linebreak.

--simple boundary--
";

        assert_eq!(
            multipart(base_mime.clone())(input),
            Ok((&b"\nThis is the epilogue. It is also to be ignored.\n"[..],
                Multipart {
                    mime: base_mime,
                    raw_part_outer: input,
                    raw_part_inner: inner,
                    children: vec![
                        AnyPart::Txt(Text {
                            mime: mime::MIME {
                                interpreted_type: mime::r#type::Deductible::Inferred(mime::r#type::Text {
                                    subtype: mime::r#type::TextSubtype::Plain,
                                    charset: mime::r#type::Deductible::Inferred(mime::charset::EmailCharset::US_ASCII),
                                }),
                                fields: mime::NaiveMIME {
                                    raw: &b"\n"[..],
                                    ..mime::NaiveMIME::default()
                                },
                            },
                            body: &b"This is implicitly typed plain US-ASCII text.\nIt does NOT end with a linebreak."[..],
                        }),
                        AnyPart::Txt(Text {
                            mime: mime::MIME { 
                                interpreted_type: mime::r#type::Deductible::Explicit(mime::r#type::Text {
                                    subtype: mime::r#type::TextSubtype::Plain,
                                    charset: mime::r#type::Deductible::Explicit(mime::charset::EmailCharset::US_ASCII),
                                }),
                                fields: mime::NaiveMIME {
                                    ctype: Some(mime::r#type::NaiveType {
                                        main: &b"text"[..],
                                        sub: &b"plain"[..],
                                        params: vec![
                                            mime::r#type::Parameter {
                                                name: &b"charset"[..],
                                                value: MIMEWord::Atom(&b"us-ascii"[..]),
                                            }
                                        ]
                                    }),
                                    raw: &b"Content-type: text/plain; charset=us-ascii\n\n"[..],
                                    ..mime::NaiveMIME::default()
                                },
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

        let hdrs = br#"Date: Sat, 8 Jul 2023 07:14:29 +0200
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

"#;

        let body = br#"This is a multi-part message in MIME format.

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
"#;

        let inner = br#"
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
"#;

        let base_mime = mime::MIME::<mime::r#type::DeductibleMessage>::default();
        assert_eq!(
            message(base_mime.clone())(fullmail),
            Ok((
                &[][..],
                Message {
                    mime: base_mime,
                    raw_part: fullmail,
                    raw_headers: hdrs,
                    raw_body: body,
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
                            interpreted_type: mime::r#type::Multipart {
                                subtype: mime::r#type::MultipartSubtype::Alternative,
                                boundary: "b1_e376dc71bafc953c0b0fdeb9983a9956".to_string(),
                            },
                            fields: mime::NaiveMIME {
                                ctype: Some(mime::r#type::NaiveType {
                                    main: &b"multipart"[..],
                                    sub: &b"alternative"[..],
                                    params: vec![
                                        mime::r#type::Parameter {
                                            name: &b"boundary"[..],
                                            value: MIMEWord::Quoted(QuotedString(vec![&b"b1_e376dc71bafc953c0b0fdeb9983a9956"[..]])),
                                        }
                                    ]
                                }),
                                raw: hdrs,
                                ..mime::NaiveMIME::default()
                            },
                        },
                        raw_part_inner: inner,
                        raw_part_outer: body,
                        children: vec![
                            AnyPart::Txt(Text {
                                mime: mime::MIME {
                                    interpreted_type: mime::r#type::Deductible::Explicit(mime::r#type::Text {
                                        subtype: mime::r#type::TextSubtype::Plain,
                                        charset: mime::r#type::Deductible::Explicit(mime::charset::EmailCharset::UTF_8),
                                    }),
                                    fields: mime::NaiveMIME {
                                        ctype: Some(mime::r#type::NaiveType {
                                            main: &b"text"[..],
                                            sub: &b"plain"[..],
                                            params: vec![
                                                mime::r#type::Parameter {
                                                    name: &b"charset"[..],
                                                    value: MIMEWord::Atom(&b"utf-8"[..]),
                                                }
                                            ]
                                        }),
                                        transfer_encoding: mime::mechanism::Mechanism::QuotedPrintable,
                                        raw: &b"Content-Type: text/plain; charset=utf-8\nContent-Transfer-Encoding: quoted-printable\n\n"[..],
                                        ..mime::NaiveMIME::default()
                                    }
                                },
                                body: &b"GZ\nOoOoO\noOoOoOoOo\noOoOoOoOoOoOoOoOo\noOoOoOoOoOoOoOoOoOoOoOo\noOoOoOoOoOoOoOoOoOoOoOoOoOoOo\nOoOoOoOoOoOoOoOoOoOoOoOoOoOoOoOoO\n"[..],
                            }),
                            AnyPart::Txt(Text {
                                mime: mime::MIME {
                                    interpreted_type: mime::r#type::Deductible::Explicit(mime::r#type::Text {
                                        subtype: mime::r#type::TextSubtype::Html,
                                        charset: mime::r#type::Deductible::Explicit(mime::charset::EmailCharset::US_ASCII),
                                    }),

                                    fields: mime::NaiveMIME {
                                        ctype: Some(mime::r#type::NaiveType {
                                            main: &b"text"[..],
                                            sub: &b"html"[..],
                                            params: vec![
                                                mime::r#type::Parameter {
                                                    name: &b"charset"[..],
                                                    value: MIMEWord::Atom(&b"us-ascii"[..]),
                                                }
                                            ]
                                        }),                             
                                        raw: &b"Content-Type: text/html; charset=us-ascii\n\n"[..],
                                        ..mime::NaiveMIME::default()
                                    },
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
