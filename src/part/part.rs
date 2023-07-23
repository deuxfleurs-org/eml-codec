use std::fmt;
use nom::{
    branch::alt,
    bytes::complete::is_not,
    combinator::{map, not, recognize},
    multi::many0,
    sequence::pair,
    IResult,
};

use crate::header::{header, CompFieldList};
use crate::mime;
use crate::mime::mime::AnyMIME;
use crate::rfc5322::{self as imf};
use crate::text::ascii::CRLF;
use crate::text::boundary::{boundary, Delimiter};
use crate::text::whitespace::obs_crlf;

#[derive(Debug, PartialEq)]
pub struct Multipart<'a>(pub mime::mime::Multipart<'a>, pub Vec<AnyPart<'a>>);

#[derive(Debug, PartialEq)]
pub struct Message<'a>(
    pub mime::mime::Message<'a>,
    pub imf::message::Message<'a>,
    pub Box<AnyPart<'a>>,
);

#[derive(PartialEq)]
pub struct Text<'a>(pub mime::mime::Text<'a>, pub &'a [u8]);
impl<'a> fmt::Debug for Text<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt
            .debug_struct("part::Text")
            .field("mime", &self.0)
            .field("body", &format_args!("\"{}\"", String::from_utf8_lossy(self.1)))
            .finish()
    }
}

#[derive(PartialEq)]
pub struct Binary<'a>(pub mime::mime::Binary<'a>, pub &'a [u8]);
impl<'a> fmt::Debug for Binary<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt
            .debug_struct("part::Binary")
            .field("mime", &self.0)
            .field("body", &format_args!("\"{}\"", String::from_utf8_lossy(self.1)))
            .finish()
    }
}

#[derive(Debug, PartialEq)]
pub enum AnyPart<'a> {
    Mult(Multipart<'a>),
    Msg(Message<'a>),
    Txt(Text<'a>),
    Bin(Binary<'a>),
}

pub enum MixedField<'a> {
    MIME(mime::field::Content<'a>),
    IMF(imf::field::Field<'a>),
}
impl<'a> MixedField<'a> {
    pub fn mime(&self) -> Option<&mime::field::Content<'a>> {
        match self {
            Self::MIME(v) => Some(v),
            _ => None,
        }
    }
    pub fn to_mime(self) -> Option<mime::field::Content<'a>> {
        match self {
            Self::MIME(v) => Some(v),
            _ => None,
        }
    }
    pub fn imf(&self) -> Option<&imf::field::Field<'a>> {
        match self {
            Self::IMF(v) => Some(v),
            _ => None,
        }
    }
    pub fn to_imf(self) -> Option<imf::field::Field<'a>> {
        match self {
            Self::IMF(v) => Some(v),
            _ => None,
        }
    }
}
impl<'a> CompFieldList<'a, MixedField<'a>> {
    pub fn sections(self) -> (mime::mime::AnyMIME<'a>, imf::message::Message<'a>) {
        let k = self.known();
        let (v1, v2): (Vec<MixedField>, Vec<MixedField>) =
            k.into_iter().partition(|v| v.mime().is_some());
        let mime = v1
            .into_iter()
            .map(|v| v.to_mime())
            .flatten()
            .collect::<mime::mime::AnyMIME>();
        let imf = v2
            .into_iter()
            .map(|v| v.to_imf())
            .flatten()
            .collect::<imf::message::Message>();
        (mime, imf)
    }
}
pub fn mixed_field(input: &[u8]) -> IResult<&[u8], MixedField> {
    alt((
        map(mime::field::content, MixedField::MIME),
        map(imf::field::field, MixedField::IMF),
    ))(input)
}

pub fn message<'a>(
    m: mime::mime::Message<'a>,
) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], Message<'a>> {
    move |input: &[u8]| {
        let (input, fields) = header(mixed_field)(input)?;
        let (in_mime, imf) = fields.sections();

        let part = to_anypart(in_mime, input);

        Ok((&[], Message(m.clone(), imf, Box::new(part))))
    }
}

pub fn multipart<'a>(
    m: mime::mime::Multipart<'a>,
) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], Multipart<'a>> {
    let m = m.clone();

    move |input| {
        let bound = m.0.boundary.as_bytes();
        let (mut input_loop, _) = part_raw(bound)(input)?;
        let mut mparts: Vec<AnyPart> = vec![];
        loop {
            let input = match boundary(bound)(input_loop) {
                Err(_) => return Ok((input_loop, Multipart(m.clone(), mparts))),
                Ok((inp, Delimiter::Last)) => return Ok((inp, Multipart(m.clone(), mparts))),
                Ok((inp, Delimiter::Next)) => inp,
            };

            // parse mime headers
            let (input, fields) = header(mime::field::content)(input)?;
            let mime = fields.to_mime();

            // parse raw part
            let (input, rpart) = part_raw(bound)(input)?;

            // parse mime body
            mparts.push(to_anypart(mime, rpart));

            input_loop = input;
        }
    }
}

pub fn to_anypart<'a>(m: AnyMIME<'a>, rpart: &'a [u8]) -> AnyPart<'a> {
    match m {
        AnyMIME::Mult(a) => map(multipart(a), AnyPart::Mult)(rpart)
            .map(|v| v.1)
            .unwrap_or(AnyPart::Txt(Text(mime::mime::Text::default(), rpart))),
        AnyMIME::Msg(a) => map(message(a), AnyPart::Msg)(rpart)
            .map(|v| v.1)
            .unwrap_or(AnyPart::Txt(Text(mime::mime::Text::default(), rpart))),
        AnyMIME::Txt(a) => AnyPart::Txt(Text(a, rpart)),
        AnyMIME::Bin(a) => AnyPart::Bin(Binary(a, rpart)),
    }
}

pub fn part_raw<'a>(bound: &[u8]) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], &'a [u8]> + '_ {
    move |input| {
        recognize(many0(pair(
            not(boundary(bound)),
            alt((is_not(CRLF), obs_crlf)),
        )))(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::encoding::{Base64Word, EncodedWord, QuotedChunk, QuotedWord};
    use crate::text::misc_token::{Phrase, UnstrToken, Unstructured, Word};
    use chrono::{FixedOffset, TimeZone};

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
            Ok((
                &b"\n--hello\nField: Body\n"[..],
                &b"blip\nbloup\n\nblip\nbloup--\n--bim\n--bim--\n"[..],
            ))
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
            Ok((
                &b"\n--simple boundary--\n"[..], 
                &b"Content-type: text/plain; charset=us-ascii\n\nThis is explicitly typed plain US-ASCII text.\nIt DOES end with a linebreak.\n"[..],
            ))
        );
    }

    #[test]
    fn test_multipart() {
        let base_mime = mime::mime::Multipart(
            mime::r#type::Multipart {
                subtype: mime::r#type::MultipartSubtype::Alternative,
                boundary: "simple boundary".to_string(),
            },
            mime::mime::Generic::default(),
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
                Multipart(
                    base_mime,
                    vec![
                        AnyPart::Txt(Text(
                            mime::mime::Text(
                                mime::r#type::Text {
                                    subtype: mime::r#type::TextSubtype::Plain,
                                    charset: mime::charset::EmailCharset::US_ASCII,
                                },
                                mime::mime::Generic::default(),
                            ),
                            &b"This is implicitly typed plain US-ASCII text.\nIt does NOT end with a linebreak."[..],
                        )),
                        AnyPart::Txt(Text(
                            mime::mime::Text(
                                mime::r#type::Text {
                                    subtype: mime::r#type::TextSubtype::Plain,
                                    charset: mime::charset::EmailCharset::US_ASCII,
                                },
                                mime::mime::Generic::default(),
                            ),
                            &b"This is explicitly typed plain US-ASCII text.\nIt DOES end with a linebreak.\n"[..],
                        )),
                    ],
                ),
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

        let base_mime = mime::mime::Message::default();
        assert_eq!(
            message(base_mime.clone())(fullmail),
            Ok((
                &[][..],
                Message (
                    base_mime,
                    imf::message::Message {
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
                        ..imf::message::Message::default()
                    },
                    Box::new(AnyPart::Mult(Multipart (
                        mime::mime::Multipart(
                            mime::r#type::Multipart {
                                subtype: mime::r#type::MultipartSubtype::Alternative,
                                boundary: "b1_e376dc71bafc953c0b0fdeb9983a9956".to_string(),
                            },
                            mime::mime::Generic::default(),
                        ),
                        vec![
                            AnyPart::Txt(Text(
                                mime::mime::Text(
                                    mime::r#type::Text {
                                        subtype: mime::r#type::TextSubtype::Plain,
                                        charset: mime::charset::EmailCharset::UTF_8,
                                    },
                                    mime::mime::Generic {
                                        transfer_encoding: mime::mechanism::Mechanism::QuotedPrintable,
                                        ..mime::mime::Generic::default()
                                    }
                                ),
                                &b"GZ\nOoOoO\noOoOoOoOo\noOoOoOoOoOoOoOoOo\noOoOoOoOoOoOoOoOoOoOoOo\noOoOoOoOoOoOoOoOoOoOoOoOoOoOo\nOoOoOoOoOoOoOoOoOoOoOoOoOoOoOoOoO\n"[..],
                            )),
                            AnyPart::Txt(Text(
                                mime::mime::Text(
                                    mime::r#type::Text {
                                        subtype: mime::r#type::TextSubtype::Html,
                                        charset: mime::charset::EmailCharset::US_ASCII,
                                    },
                                    mime::mime::Generic::default(),
                                ),
                                &br#"<div style="text-align: center;"><strong>GZ</strong><br />
OoOoO<br />
oOoOoOoOo<br />
oOoOoOoOoOoOoOoOo<br />
oOoOoOoOoOoOoOoOoOoOoOo<br />
oOoOoOoOoOoOoOoOoOoOoOoOoOoOo<br />
OoOoOoOoOoOoOoOoOoOoOoOoOoOoOoOoO<br />
</div>
"#[..],
                            )),
                        ],
                    ))),
                ),
            ))
        );
    }
}
