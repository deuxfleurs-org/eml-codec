use nom::IResult;

use crate::header::{header, self};
use crate::imf;
use crate::mime;
use crate::part::{self, AnyPart, field::MixedField};
use crate::text::boundary::{boundary, Delimiter};

//--- Multipart
#[derive(Debug, PartialEq)]
pub struct Multipart<'a> {
    pub interpreted: mime::MIME<'a, mime::r#type::Multipart>,
    pub children: Vec<AnyPart<'a>>,
    pub preamble: &'a [u8],
    pub epilogue: &'a [u8],
}
impl<'a> Multipart<'a> {
    pub fn with_epilogue(mut self, e: &'a [u8]) -> Self {
        self.epilogue = e;
        self
    }
}

pub fn multipart<'a>(
    m: mime::MIME<'a, mime::r#type::Multipart>,
) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], Multipart<'a>> {
    let m = m.clone();

    move |input| {
        let bound = m.interpreted.boundary.as_bytes();
        let (mut input_loop, preamble) = part::part_raw(bound)(input)?;
        let mut mparts: Vec<AnyPart> = vec![];
        loop {
            let input = match boundary(bound)(input_loop) {
                Err(_) => {
                    return Ok((
                        input_loop,
                        Multipart {
                            interpreted: m.clone(),
                            children: mparts,
                            preamble,
                            epilogue: &[],
                        },
                    ))
                }
                Ok((inp, Delimiter::Last)) => {
                    return Ok((
                        inp,
                        Multipart {
                            interpreted: m.clone(),
                            children: mparts,
                            preamble,
                            epilogue: &[],
                        },
                    ))
                }
                Ok((inp, Delimiter::Next)) => inp,
            };

            // parse mime headers, otherwise pick default mime
            let (input, naive_mime) = match header(mime::field::content)(input) {
                Ok((input, (known, unknown, bad))) => (input, known.into_iter().collect::<mime::NaiveMIME>().with_opt(unknown).with_bad(bad)),
                Err(_) => (input, mime::NaiveMIME::default()),
            };

            // interpret mime according to context
            let mime = match m.interpreted.subtype {
                mime::r#type::MultipartSubtype::Digest => naive_mime.to_interpreted::<mime::WithDigestDefault>().into(),
                _ => naive_mime.to_interpreted::<mime::WithGenericDefault>().into(),
            };

            // parse raw part
            let (input, rpart) = part::part_raw(bound)(input)?;

            // parse mime body
            mparts.push(part::to_anypart(mime, rpart));


            input_loop = input;
        }
    }
}

//--- Message

#[derive(Debug, PartialEq)]
pub struct Message<'a> {
    pub interpreted: mime::MIME<'a, mime::r#type::Message>,
    pub imf: imf::Imf<'a>,
    pub child: Box<AnyPart<'a>>,
    pub epilogue: &'a [u8],
}
impl<'a> Message<'a> {
    pub fn with_epilogue(mut self, e: &'a [u8]) -> Self {
        self.epilogue = e;
        self
    }
} 

pub fn message<'a>(
    m: mime::MIME<'a, mime::r#type::Message>,
) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], Message<'a>> {
    move |input: &[u8]| {
        // parse header fields
        let (input, (known, unknown, bad)): (_, (Vec::<MixedField>, Vec<header::Kv>, Vec<&[u8]>)) =
            header(part::field::mixed_field)(input)?;

        // aggregate header fields
        let (naive_mime, imf) = part::field::sections(known);

        // attach bad headers to imf
        let imf = imf.with_opt(unknown).with_bad(bad);

        // interpret headers to choose a mime type
        let in_mime = naive_mime.to_interpreted::<mime::WithGenericDefault>().into();

        // parse this mimetype
        let part = part::to_anypart(in_mime, input);

        Ok((
            &[],
            Message {
                interpreted: m.clone(),
                imf,
                child: Box::new(part),
                epilogue: &[],
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
            interpreted: mime::r#type::Multipart {
                subtype: mime::r#type::MultipartSubtype::Alternative,
                boundary: "simple boundary".to_string(),
            },
            parsed: mime::NaiveMIME::default(),
        };

        assert_eq!(
            multipart(base_mime.clone())(b"This is the preamble.  It is to be ignored, though it
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
"),
            Ok((&b"\nThis is the epilogue. It is also to be ignored.\n"[..],
                Multipart {
                    interpreted: base_mime,
                    preamble: &b"This is the preamble.  It is to be ignored, though it\nis a handy place for composition agents to include an\nexplanatory note to non-MIME conformant readers.\n"[..],
                    epilogue: &b""[..],
                    children: vec![
                        AnyPart::Txt(Text {
                            interpreted: mime::MIME {
                                interpreted: mime::r#type::Text {
                                    subtype: mime::r#type::TextSubtype::Plain,
                                    charset: mime::charset::EmailCharset::US_ASCII,
                                },
                                parsed: mime::NaiveMIME::default(),
                            },
                            body: &b"This is implicitly typed plain US-ASCII text.\nIt does NOT end with a linebreak."[..],
                        }),
                        AnyPart::Txt(Text {
                            interpreted: mime::MIME { 
                                interpreted: mime::r#type::Text {
                                    subtype: mime::r#type::TextSubtype::Plain,
                                    charset: mime::charset::EmailCharset::US_ASCII,
                                },
                                parsed: mime::NaiveMIME {
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

        let base_mime = mime::MIME::<mime::r#type::Message>::default();
        assert_eq!(
            message(base_mime.clone())(fullmail),
            Ok((
                &[][..],
                Message {
                    interpreted: base_mime,
                    epilogue: &b""[..],
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
                        header_ext: vec![
                            header::Kv(&b"X-Unknown"[..], Unstructured(vec![
                                UnstrToken::Plain(&b"something"[..]),
                                UnstrToken::Plain(&b"something"[..]),
                            ]))
                        ],
                        header_bad: vec![
                            &b"Bad entry\n  on multiple lines\n"[..],
                        ],
                        ..imf::Imf::default()
                    },
                    child: Box::new(AnyPart::Mult(Multipart {
                        interpreted: mime::MIME {
                            interpreted: mime::r#type::Multipart {
                                subtype: mime::r#type::MultipartSubtype::Alternative,
                                boundary: "b1_e376dc71bafc953c0b0fdeb9983a9956".to_string(),
                            },
                            parsed: mime::NaiveMIME {
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
                                ..mime::NaiveMIME::default()
                            },
                        },
                        preamble: &b"This is a multi-part message in MIME format.\n"[..],
                        epilogue: &b""[..],
                        children: vec![
                            AnyPart::Txt(Text {
                                interpreted: mime::MIME {
                                    interpreted: mime::r#type::Text {
                                        subtype: mime::r#type::TextSubtype::Plain,
                                        charset: mime::charset::EmailCharset::UTF_8,
                                    },
                                    parsed: mime::NaiveMIME {
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
                                        ..mime::NaiveMIME::default()
                                    }
                                },
                                body: &b"GZ\nOoOoO\noOoOoOoOo\noOoOoOoOoOoOoOoOo\noOoOoOoOoOoOoOoOoOoOoOo\noOoOoOoOoOoOoOoOoOoOoOoOoOoOo\nOoOoOoOoOoOoOoOoOoOoOoOoOoOoOoOoO\n"[..],
                            }),
                            AnyPart::Txt(Text {
                                interpreted: mime::MIME {
                                    interpreted: mime::r#type::Text {
                                        subtype: mime::r#type::TextSubtype::Html,
                                        charset: mime::charset::EmailCharset::US_ASCII,
                                    },

                                    parsed: mime::NaiveMIME {
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
