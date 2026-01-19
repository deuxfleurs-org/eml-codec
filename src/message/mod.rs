use bounded_static::ToStatic;
use nom::IResult;

use crate::header;
use crate::imf;
use crate::mime;
use crate::part;
use crate::print::{Print, Formatter};

/// A complete **toplevel message**.
/// This represent a complete "email" that can be send and received over the wire, for example.
#[derive(Clone, Debug, PartialEq, ToStatic)]
pub struct Message<'a> {
    pub imf: imf::Imf<'a>,
    pub mime_body: part::MimeBody<'a>,
    pub all_fields: Vec<MessageField<'a>>,
}

impl<'a> Print for Message<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        fmt.begin_line_folding();
        let mime = self.mime_body.mime();
        for field in &self.all_fields {
            match field {
                MessageField::Unstructured(u) => u.print(fmt),
                MessageField::MIME(f) => mime.print_field(*f, fmt),
                MessageField::Imf(f) => self.imf.print_field(*f, fmt),
            }
        }
        fmt.end_line_folding();
        fmt.write_crlf();
        self.mime_body.print_body(fmt);
    }
}

pub fn message<'a>(input: &'a [u8]) -> IResult<&'a [u8], Message<'a>> {
    // parse headers
    let (input_body, headers) = header::header_kv(input)?;
    let fields: MessageFields =
        headers.into_iter().collect::<Option<MessageFields>>()
        .ok_or(
            nom::Err::Failure(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Verify, // FIXME: output the actual error
            )))?;

    let (input_end, mime_body) =
        part::part_body(fields.mime.to_interpreted(mime::DefaultType::Generic))(input_body)?;
    // note: part_body always consumes the whole input
    debug_assert!(input_end.is_empty());
    Ok((input_end, Message {
        imf: fields.imf,
        mime_body,
        all_fields: fields.all_fields,
    }))
}

pub fn imf<'a>(input: &'a [u8]) -> IResult<&'a [u8], imf::Imf<'a>> {
    // parse headers
    let (input_body, headers) = header::header_kv(input)?;
    let fields: MessageFields =
        headers.into_iter().collect::<Option<MessageFields>>()
        .ok_or(
            nom::Err::Failure(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Verify, // FIXME: output the actual error
            )))?;
    Ok((input_body, fields.imf))
}

/// Header field of a toplevel message.
/// Is either an Imf field (RFC 5322),
/// MIME-defined fields (RFC 2045),
/// or an unstructured field.
#[derive(Clone, Debug, PartialEq, ToStatic)]
pub enum MessageField<'a> {
    MIME(mime::field::Entry),
    Imf(imf::field::Entry),
    Unstructured(header::Unstructured<'a>),
}

#[derive(Debug, PartialEq, ToStatic)]
struct MessageFields<'a> {
    mime: mime::NaiveMIME<'a>,
    imf: imf::Imf<'a>,
    // trace fields guaranteed to occur before all other IMF fields
    all_fields: Vec<MessageField<'a>>,
}

impl<'a> FromIterator<header::FieldRaw<'a>> for Option<MessageFields<'a>> {
    fn from_iter<I: IntoIterator<Item = header::FieldRaw<'a>>>(it: I) -> Self {
        let mut mime = mime::NaiveMIME::default();
        let mut imf = imf::PartialImf::default();
        let mut all_fields = vec![];
        for f in it {
            if let Ok(mimef) = mime::field::Content::try_from(&f) {
                if let Some(entry) = mime.add_field(mimef) {
                    all_fields.push(MessageField::MIME(entry))
                } // otherwise drop the field
                continue;
            }

            if let Ok(imff) = imf::field::Field::try_from(&f) {
                if let Some(entry) = imf.add_field(imff) {
                    all_fields.push(MessageField::Imf(entry))
                } // otherwise drop the field
                continue;
            }

            if let Some(u) = header::Unstructured::from_raw(f) {
                all_fields.push(MessageField::Unstructured(u));
            } // otherwise drop the field
        }

        imf.to_imf().map(|imf| MessageFields { mime, imf, all_fields })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::imf::datetime::DateTime;
    use crate::imf::{Imf, From};
    use crate::imf::address::*;
    use crate::imf::identification::MessageIDRight;
    use crate::imf::mailbox::*;
    use crate::mime::{CommonMIME, MIME};
    use crate::mime::r#type::Deductible;
    use crate::part::composite::Multipart;
    use crate::part::discrete::Text;
    use crate::part::{AnyPart, MimeBody};
    use crate::part::field::EntityField;
    use crate::print::tests::with_formatter;
    use crate::text::encoding::{Base64Word, EncodedWord, EncodedWordToken, QuotedChunk, QuotedWord};
    use crate::text::misc_token::*;
    use chrono::{FixedOffset, TimeZone};
    use pretty_assertions::assert_eq;

    fn test_message_roundtrip<'a>(txt: &[u8], parsed: Message<'a>) {
        assert_eq!(message(txt), Ok((&b""[..], parsed.clone())));
        let printed = with_formatter(|fmt| parsed.print(fmt));
        assert_eq!(String::from_utf8_lossy(&printed), String::from_utf8_lossy(txt))
    }

    fn test_message_parse_print<'a>(txt: &[u8], parsed: Message<'a>, printed: &[u8]) {
        assert_eq!(message(txt), Ok((&b""[..], parsed.clone())));
        let reprinted = with_formatter(|fmt| parsed.print(fmt));
        assert_eq!(String::from_utf8_lossy(&reprinted), String::from_utf8_lossy(printed))
    }

    #[test]
    fn test_simple() {
        let fullmail = b"Date: 7 Mar 2023 08:00:00 +0200\r
From: someone@example.com\r
To: someone_else@example.com\r
Subject: An  RFC 822  formatted message\r
\r
This is the plain text body of the message. Note the blank line
between the header information and the body of the message.";

        test_message_roundtrip(
            fullmail,
            {
                let from = MailboxRef {
                    name: None,
                    addrspec: AddrSpec {
                        local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(b"someone"[..].into()))]),
                        domain: Domain::Atoms(vec![b"example"[..].into(), b"com"[..].into()]),
                    }
                };
                let mut imf = Imf::new(
                    From::Single { from, sender: None },
                    DateTime(FixedOffset::east_opt(2 * 3600).unwrap().with_ymd_and_hms(2023, 3, 7, 8, 0, 0).unwrap()),
                );
                imf.to = vec![AddressRef::Single(MailboxRef {
                    name: None,
                    addrspec: AddrSpec {
                        local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(b"someone_else"[..].into()))]),
                        domain: Domain::Atoms(vec![b"example"[..].into(), b"com"[..].into()]),
                    }
                })];
                imf.subject = Some(Unstructured(vec![
                    UnstrToken::from_plain(b" ", UnstrTxtKind::Fws),
                    UnstrToken::from_plain(b"An", UnstrTxtKind::Txt),
                    UnstrToken::from_plain(b"  ", UnstrTxtKind::Fws),
                    UnstrToken::from_plain(b"RFC", UnstrTxtKind::Txt),
                    UnstrToken::from_plain(b" ", UnstrTxtKind::Fws),
                    UnstrToken::from_plain(b"822", UnstrTxtKind::Txt),
                    UnstrToken::from_plain(b"  ", UnstrTxtKind::Fws),
                    UnstrToken::from_plain(b"formatted", UnstrTxtKind::Txt),
                    UnstrToken::from_plain(b" ", UnstrTxtKind::Fws),
                    UnstrToken::from_plain(b"message", UnstrTxtKind::Txt),
                ]));

                let mime_body = part::MimeBody::Txt(
                    part::discrete::Text {
                        mime: MIME {
                            ctype: Deductible::Inferred(mime::r#type::Text::default()),
                            fields: CommonMIME::default(),
                        },
                        body: b"This is the plain text body of the message. Note the blank line\nbetween the header information and the body of the message."[..].into(),
                    }
                );

                let all_fields = vec![
                    MessageField::Imf(imf::field::Entry::Date),
                    MessageField::Imf(imf::field::Entry::From),
                    MessageField::Imf(imf::field::Entry::To),
                    MessageField::Imf(imf::field::Entry::Subject),
                ];

                Message {
                    imf,
                    mime_body,
                    all_fields,
                }
            }
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
X-Custom: foobar
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

        let ast =
            Message {
                    imf: {
                        let from = imf::mailbox::MailboxRef {
                                name: Some(Phrase(vec![
                                    PhraseToken::Word(Word::Atom(b"Grrrnd"[..].into())),
                                    PhraseToken::Word(Word::Atom(b"Zero"[..].into())),
                                ])),
                                addrspec: imf::mailbox::AddrSpec {
                                    local_part: imf::mailbox::LocalPart(vec![
                                        imf::mailbox::LocalPartToken::Word(Word::Atom(b"grrrndzero"[..].into()))
                                    ]),
                                    domain: imf::mailbox::Domain::Atoms(vec![b"example"[..].into(), b"org"[..].into()]),
                                }
                            };
                        let date = imf::datetime::DateTime(FixedOffset::east_opt(2 * 3600)
                            .unwrap()
                            .with_ymd_and_hms(2023, 07, 8, 7, 14, 29)
                            .unwrap());

                        let mut imf = imf::Imf::new(imf::From::Single { from, sender: None }, date);

                        imf.to = vec![imf::address::AddressRef::Single(imf::mailbox::MailboxRef {
                                name: Some(Phrase(vec![
                                    PhraseToken::Word(Word::Atom(b"John"[..].into())),
                                    PhraseToken::Word(Word::Atom(b"Doe"[..].into())),
                                ])),
                                addrspec: imf::mailbox::AddrSpec {
                                    local_part: imf::mailbox::LocalPart(vec![
                                        imf::mailbox::LocalPartToken::Word(Word::Atom(b"jdoe"[..].into()))
                                    ]),
                                    domain: imf::mailbox::Domain::Atoms(vec![b"machine"[..].into(), b"example"[..].into()]),
                                }
                         })];

                        imf.cc = vec![imf::address::AddressRef::Single(imf::mailbox::MailboxRef {
                            name: Some(Phrase(vec![
                                PhraseToken::Encoded(EncodedWord(vec![
                                    EncodedWordToken::Quoted(QuotedWord {
                                        enc: encoding_rs::WINDOWS_1252,
                                        chunks: vec![
                                            QuotedChunk::Safe(b"Andr"[..].into()),
                                            QuotedChunk::Encoded(vec![0xE9]),
                                        ],
                                    })
                                ])),
                                PhraseToken::Word(Word::Atom(b"Pirard"[..].into())),
                            ])),
                            addrspec: imf::mailbox::AddrSpec {
                                local_part: imf::mailbox::LocalPart(vec![
                                    imf::mailbox::LocalPartToken::Word(Word::Atom(b"PIRARD"[..].into()))
                                ]),
                                domain: imf::mailbox::Domain::Atoms(vec![
                                    b"vm1"[..].into(), b"ulg"[..].into(), b"ac"[..].into(), b"be"[..].into(),
                                ]),
                            }
                        })];

                        imf.subject = Some(Unstructured(vec![
                            UnstrToken::from_plain(b" ", UnstrTxtKind::Fws),
                            UnstrToken::Encoded(EncodedWord(vec![
                                EncodedWordToken::Base64(Base64Word{
                                    enc: encoding_rs::WINDOWS_1252,
                                    content: b"SWYgeW91IGNhbiByZWFkIHRoaXMgeW8"[..].into(),
                                }),
                                EncodedWordToken::Base64(Base64Word{
                                    enc: encoding_rs::ISO_8859_2,
                                    content: b"dSB1bmRlcnN0YW5kIHRoZSBleGFtcGxlLg"[..].into(),
                                })
                            ])),
                        ]));

                        imf.msg_id = Some(imf::identification::MessageID {
                            left: b"NTAxNzA2AC47634Y366BAMTY4ODc5MzQyODY0ODY5"[..].into(),
                            right: MessageIDRight::DotAtom(b"www.grrrndzero.org"[..].into()),
                        });

                        imf
                    },
                    all_fields: vec![
                        MessageField::Imf(imf::field::Entry::Date),
                        MessageField::Imf(imf::field::Entry::From),
                        MessageField::Imf(imf::field::Entry::To),
                        MessageField::Imf(imf::field::Entry::Cc),
                        MessageField::Imf(imf::field::Entry::Subject),
                        MessageField::Unstructured(
                            header::Unstructured(
                                header::FieldName(b"X-Unknown"[..].into()),
                                Unstructured(vec![
                                    UnstrToken::from_plain(b" ", UnstrTxtKind::Fws),
                                    UnstrToken::from_plain(b"something", UnstrTxtKind::Txt),
                                    UnstrToken::from_plain(b" ", UnstrTxtKind::Fws),
                                    UnstrToken::from_plain(b"something", UnstrTxtKind::Txt),
                                ]),
                            )
                        ),
                        MessageField::Imf(imf::field::Entry::MessageId),
                        MessageField::Imf(imf::field::Entry::MIMEVersion),
                        MessageField::MIME(mime::field::Entry::Type),
                        MessageField::MIME(mime::field::Entry::TransferEncoding),
                    ],
                    mime_body: MimeBody::Mult(Multipart {
                        mime: mime::MIME {
                            ctype: mime::r#type::Multipart {
                                subtype: mime::r#type::MultipartSubtype::Alternative,
                                boundary: Some(b"b1_e376dc71bafc953c0b0fdeb9983a9956".to_vec()),
                                params: vec![],
                            },
                            fields: mime::CommonMIME::default(),
                        },
                        preamble: preamble.into(),
                        epilogue: vec![].into(),
                        children: vec![
                            AnyPart {
                                fields: vec![
                                    EntityField::MIME(mime::field::Entry::Type),
                                    EntityField::MIME(mime::field::Entry::TransferEncoding),
                                ],
                                mime_body: MimeBody::Txt(Text {
                                    mime: mime::MIME {
                                        ctype: mime::r#type::Deductible::Explicit(mime::r#type::Text {
                                            subtype: mime::r#type::TextSubtype::Plain,
                                            charset: mime::r#type::Deductible::Explicit(mime::charset::EmailCharset::UTF_8),
                                            params: vec![],
                                        }),
                                        fields: mime::CommonMIME {
                                            transfer_encoding: mime::mechanism::Mechanism::QuotedPrintable,
                                            ..mime::CommonMIME::default()
                                        }
                                    },
                                    body: b"GZ\nOoOoO\noOoOoOoOo\noOoOoOoOoOoOoOoOo\noOoOoOoOoOoOoOoOoOoOoOo\noOoOoOoOoOoOoOoOoOoOoOoOoOoOo\nOoOoOoOoOoOoOoOoOoOoOoOoOoOoOoOoO\n"[..].into(),
                                }),
                            },
                            AnyPart {
                                fields: vec![
                                    EntityField::Unstructured(header::Unstructured(
                                        header::FieldName(b"X-Custom".into()),
                                        Unstructured(vec![
                                            UnstrToken::from_plain(b" ", UnstrTxtKind::Fws),
                                            UnstrToken::from_plain(b"foobar", UnstrTxtKind::Txt),
                                        ])
                                    )),
                                    EntityField::MIME(mime::field::Entry::Type),
                                ],
                                mime_body: MimeBody::Txt(Text {
                                    mime: mime::MIME {
                                        ctype: mime::r#type::Deductible::Explicit(mime::r#type::Text {
                                            subtype: mime::r#type::TextSubtype::Html,
                                            charset: mime::r#type::Deductible::Explicit(mime::charset::EmailCharset::US_ASCII),
                                            params: vec![],
                                        }),

                                        fields: mime::CommonMIME::default(),
                                    },
                                    body: br#"<div style="text-align: center;"><strong>GZ</strong><br />
OoOoO<br />
oOoOoOoOo<br />
oOoOoOoOoOoOoOoOo<br />
oOoOoOoOoOoOoOoOoOoOoOo<br />
oOoOoOoOoOoOoOoOoOoOoOoOoOoOo<br />
OoOoOoOoOoOoOoOoOoOoOoOoOoOoOoOoO<br />
</div>
"#[..].into(),
                                }),
                            },
                        ],
                    })
                };

        let reprinted: &[u8] = "Date: 8 Jul 2023 07:14:29 +0200\r
From: Grrrnd Zero <grrrndzero@example.org>\r
To: John Doe <jdoe@machine.example>\r
Cc: =?UTF-8?Q?Andr=C3=A9?= Pirard <PIRARD@vm1.ulg.ac.be>\r
Subject: =?UTF-8?Q?If_you_can_read_this_yo?=\r
 =?UTF-8?Q?u_understand_the_example=2E?=\r
X-Unknown: something something\r
Message-ID: <NTAxNzA2AC47634Y366BAMTY4ODc5MzQyODY0ODY5@www.grrrndzero.org>\r
MIME-Version: 1.0\r
Content-Type: multipart/alternative;\r
 boundary=\"V1Qy0rpB5tWE76WF3UelfGW5K9LZpjHjZ3PKE26vpVNnvofq7BLuYTWxzQB3HrYu7\"\r
Content-Transfer-Encoding: 7bit\r
\r
--V1Qy0rpB5tWE76WF3UelfGW5K9LZpjHjZ3PKE26vpVNnvofq7BLuYTWxzQB3HrYu7\r
Content-Type: text/plain; charset=UTF-8\r
Content-Transfer-Encoding: quoted-printable\r
\r
GZ
OoOoO
oOoOoOoOo
oOoOoOoOoOoOoOoOo
oOoOoOoOoOoOoOoOoOoOoOo
oOoOoOoOoOoOoOoOoOoOoOoOoOoOo
OoOoOoOoOoOoOoOoOoOoOoOoOoOoOoOoO
\r
--V1Qy0rpB5tWE76WF3UelfGW5K9LZpjHjZ3PKE26vpVNnvofq7BLuYTWxzQB3HrYu7\r
X-Custom: foobar\r
Content-Type: text/html; charset=US-ASCII\r
\r
<div style=\"text-align: center;\"><strong>GZ</strong><br />
OoOoO<br />
oOoOoOoOo<br />
oOoOoOoOoOoOoOoOo<br />
oOoOoOoOoOoOoOoOoOoOoOo<br />
oOoOoOoOoOoOoOoOoOoOoOoOoOoOo<br />
OoOoOoOoOoOoOoOoOoOoOoOoOoOoOoOoO<br />
</div>
\r
--V1Qy0rpB5tWE76WF3UelfGW5K9LZpjHjZ3PKE26vpVNnvofq7BLuYTWxzQB3HrYu7--\r
"
        .as_bytes();

        test_message_parse_print(fullmail, ast, reprinted);
    }
}
