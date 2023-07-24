use nom::IResult;

use crate::header::{header, self};
use crate::imf;
use crate::mime;
use crate::part::{self, AnyPart, field::MixedField};
use crate::text::boundary::{boundary, Delimiter};

//--- Multipart
#[derive(Debug, PartialEq)]
pub struct Multipart<'a> {
    pub interpreted: mime::Multipart<'a>,
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
    m: mime::Multipart<'a>,
) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], Multipart<'a>> {
    let m = m.clone();

    move |input| {
        let bound = m.0.boundary.as_bytes();
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

            // parse mime headers
            let (input, (known, unknown, bad)) = header(mime::field::content)(input)?;
            let mime = match m.0.subtype {
                mime::r#type::MultipartSubtype::Digest => mime::field::to_mime::<mime::WithDigestDefault>(known).into(),
                _ => mime::field::to_mime::<mime::WithGenericDefault>(known).into(),
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
    pub interpreted: mime::Message<'a>,
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
    m: mime::Message<'a>,
) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], Message<'a>> {
    move |input: &[u8]| {
        let (input, (known, unknown, bad)): (_, (Vec::<MixedField>, Vec<header::Kv>, Vec<&[u8]>)) =
            header(part::field::mixed_field)(input)?;
        let (in_mime, imf) = part::field::sections::<mime::WithGenericDefault>(known);

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
    use crate::text::misc_token::{Phrase, UnstrToken, Unstructured, Word};
    use chrono::{FixedOffset, TimeZone};

    #[test]
    fn test_multipart() {
        let base_mime = mime::Multipart(
            mime::r#type::Multipart {
                subtype: mime::r#type::MultipartSubtype::Alternative,
                boundary: "simple boundary".to_string(),
            },
            mime::Generic::default(),
        );

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
                            interpreted: mime::Text(
                                mime::r#type::Text {
                                    subtype: mime::r#type::TextSubtype::Plain,
                                    charset: mime::charset::EmailCharset::US_ASCII,
                                },
                                mime::Generic::default(),
                            ),
                            body: &b"This is implicitly typed plain US-ASCII text.\nIt does NOT end with a linebreak."[..],
                        }),
                        AnyPart::Txt(Text {
                            interpreted: mime::Text(
                                mime::r#type::Text {
                                    subtype: mime::r#type::TextSubtype::Plain,
                                    charset: mime::charset::EmailCharset::US_ASCII,
                                },
                                mime::Generic::default(),
                            ),
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

        let base_mime = mime::Message::default();
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
                        ..imf::Imf::default()
                    },
                    child: Box::new(AnyPart::Mult(Multipart {
                        interpreted: mime::Multipart(
                            mime::r#type::Multipart {
                                subtype: mime::r#type::MultipartSubtype::Alternative,
                                boundary: "b1_e376dc71bafc953c0b0fdeb9983a9956".to_string(),
                            },
                            mime::Generic::default(),
                        ),
                        preamble: &b"This is a multi-part message in MIME format.\n"[..],
                        epilogue: &b""[..],
                        children: vec![
                            AnyPart::Txt(Text {
                                interpreted: mime::Text(
                                    mime::r#type::Text {
                                        subtype: mime::r#type::TextSubtype::Plain,
                                        charset: mime::charset::EmailCharset::UTF_8,
                                    },
                                    mime::Generic {
                                        transfer_encoding: mime::mechanism::Mechanism::QuotedPrintable,
                                        ..mime::Generic::default()
                                    }
                                ),
                                body: &b"GZ\nOoOoO\noOoOoOoOo\noOoOoOoOoOoOoOoOo\noOoOoOoOoOoOoOoOoOoOoOo\noOoOoOoOoOoOoOoOoOoOoOoOoOoOo\nOoOoOoOoOoOoOoOoOoOoOoOoOoOoOoOoO\n"[..],
                            }),
                            AnyPart::Txt(Text {
                                interpreted: mime::Text(
                                    mime::r#type::Text {
                                        subtype: mime::r#type::TextSubtype::Html,
                                        charset: mime::charset::EmailCharset::US_ASCII,
                                    },
                                    mime::Generic::default(),
                                ),
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
