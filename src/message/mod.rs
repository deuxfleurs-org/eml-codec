#[cfg(feature = "arbitrary")]
use arbitrary::Arbitrary;
use bounded_static::ToStatic;
#[cfg(feature = "tracing")]
use tracing::warn;

#[cfg(feature = "arbitrary")]
use crate::{
    arbitrary_utils::{arbitrary_shuffle, arbitrary_vec_where},
    fuzz_eq::FuzzEq,
    imf::Imf,
    part::MimeBody,
};
use crate::header;
use crate::imf;
use crate::mime;
use crate::part;
use crate::print::{Print, Formatter};

/// A complete **toplevel message**.
/// This represent a complete "email" that can be send and received over the wire, for example.
#[derive(Clone, Debug, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub struct Message<'a> {
    // Invariant: `all_fields` must contain an entry for every piece of information
    // contained in `imf` and `mime_body`'s mime headers that is mandatory or is
    // not the default value..
    // Invariant: IMF trace fields must occur before any other IMF or MIME fields.
    // Invariant: the indices of Trace, Comments and Keywords entries occur in-order
    // (0, 1, ...). In other words, it is the respective Vec in `imf` that contain
    // the referenced data that define the order).
    pub imf: imf::Imf<'a>,
    pub mime_body: part::MimeBody<'a>,
    pub entries: Vec<MessageEntry<'a>>,
}

impl<'a> Print for Message<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        fmt.begin_line_folding();
        let mime = self.mime_body.mime();
        for entry in &self.entries {
            match entry {
                MessageEntry::Unstructured(u) => u.print(fmt),
                MessageEntry::MIME(f) => mime.print_field(*f, fmt),
                MessageEntry::Imf(f) => self.imf.print_field(*f, fmt),
            }
        }
        fmt.end_line_folding();
        fmt.write_crlf();
        self.mime_body.print_body(fmt);
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for Message<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let imf: Imf = u.arbitrary()?;
        let (trace_entries, imf_entries) = imf.field_entries();
        let mime_body: MimeBody = u.arbitrary()?;

        fn arbitrary_unstructured<'a>(u: &mut arbitrary::Unstructured<'a>) ->
            arbitrary::Result<Vec<header::Unstructured<'a>>>
        {
            arbitrary_vec_where(u, |f: &header::Unstructured| {
                !imf::field::is_imf_header(&f.name) && !mime::field::is_mime_header(&f.name)
            })
        }

        // compute the trace section (which includes unstructured headers)
        let mut entries: Vec<_> = trace_entries.into_iter().map(MessageEntry::Imf).collect();
        entries.extend(arbitrary_unstructured(u)?.into_iter().map(MessageEntry::Unstructured));
        arbitrary_shuffle(u, &mut entries)?;
        // Renumber Trace entries so that their index is in order.
        {
            let mut id = 0;
            for e in entries.iter_mut() {
                if let MessageEntry::Imf(imf::field::Entry::Trace(_)) = e {
                    *e = MessageEntry::Imf(imf::field::Entry::Trace(id));
                    id += 1
                }
            }
        }

        // compute the rest
        let mut rest: Vec<MessageEntry> =
            mime_body.mime()
                     .field_entries()
                     .into_iter()
                     .map(MessageEntry::MIME)
                     .collect();
        rest.extend(imf_entries.into_iter().map(MessageEntry::Imf));
        rest.extend(arbitrary_unstructured(u)?.into_iter().map(MessageEntry::Unstructured));
        arbitrary_shuffle(u, &mut rest)?;
        // Renumber `Comments` and `Keywords` entries.
        {
            let mut comments_id = 0;
            let mut keywords_id = 0;
            for e in rest.iter_mut() {
                if let MessageEntry::Imf(imf::field::Entry::Comments(_)) = e {
                    *e = MessageEntry::Imf(imf::field::Entry::Comments(comments_id));
                    comments_id += 1
                } else if let MessageEntry::Imf(imf::field::Entry::Keywords(_)) = e {
                    *e = MessageEntry::Imf(imf::field::Entry::Keywords(keywords_id));
                    keywords_id += 1
                }
            }
        }

        // concatenate both sections
        entries.extend(rest.into_iter());

        Ok(Message { imf, mime_body, entries })
    }
}

/// Parse a toplevel message.
pub fn message<'a>(input: &'a [u8]) -> Message<'a> {
    // parse headers
    let (input_body, headers) = header::header_kv(input);
    let fields: MessageFields = headers.into_iter().collect::<MessageFields>();
    let mime = fields.mime.to_interpreted(mime::DefaultType::Generic);
    // parse body
    let mime_body = part::part_body(mime)(input_body);
    Message {
        imf: fields.imf,
        mime_body,
        entries: fields.entries,
    }
}

pub fn imf<'a>(input: &'a [u8]) -> (&'a [u8], imf::Imf<'a>) {
    // parse headers
    let (input_body, headers) = header::header_kv(input);
    let fields: MessageFields = headers.into_iter().collect::<MessageFields>();
    (input_body, fields.imf)
}

/// Header field of a toplevel message.
/// Is either an Imf field (RFC 5322),
/// MIME-defined fields (RFC 2045),
/// or an unstructured field.
#[derive(Clone, Debug, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub enum MessageEntry<'a> {
    MIME(mime::field::Entry),
    Imf(imf::field::Entry),
    // invariant: has a field name that is different from IMF or MIME headers.
    Unstructured(header::Unstructured<'a>),
}

#[derive(Debug, PartialEq, ToStatic)]
struct MessageFields<'a> {
    mime: mime::NaiveMIME<'a>,
    imf: imf::Imf<'a>,
    entries: Vec<MessageEntry<'a>>,
}

impl<'a> FromIterator<header::FieldRaw<'a>> for MessageFields<'a> {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", name = "MessageFields::from_iter", skip(it))
    )]
    fn from_iter<I: IntoIterator<Item = header::FieldRaw<'a>>>(it: I) -> Self {
        let mut mime = mime::NaiveMIME::default();
        let mut imf = imf::PartialImf::default();
        let mut entries = vec![];
        for f in it {
            match mime::field::Content::try_from(&f) {
                Ok(mimef) => {
                    if let Some(entry) = mime.add_field(mimef) {
                        entries.push(MessageEntry::MIME(entry))
                    } else {
                        // otherwise drop the field
                        #[cfg(feature = "tracing-recover")]
                        warn!(field = ?f, "dropping conflicting MIME field")
                    }
                    continue;
                },
                Err(mime::field::InvalidField::Body) => {
                    // this is a MIME field but its body is invalid; drop it.
                    #[cfg(feature = "tracing-discard")]
                    warn!(field = ?f, "dropping MIME field with an invalid body");
                    continue;
                },
                Err(mime::field::InvalidField::Name) => {
                    // not a MIME field
                    ()
                }
            };

            match imf::field::Field::try_from(&f) {
                Ok(imff) => {
                    match imf.add_field(imff) {
                        Ok(entry) =>
                            entries.push(MessageEntry::Imf(entry)),
                        Err(imf::AddFieldErr::NoEntry) => {
                            #[cfg(feature = "tracing-recover")]
                            warn!(field = ?f, "no new entry for IMF field");
                        },
                        Err(imf::AddFieldErr::Conflict) => {
                            #[cfg(feature = "tracing-recover")]
                            warn!(field = ?f, "discarding conflicting IMF field");
                        },
                    }
                    continue;
                },
                Err(imf::field::InvalidField::NeedsDiscard) => {
                    // this is an IMF field for which we recognized the body, but the
                    // body isn't RFC compliant and the fields needs to be dropped.
                    #[cfg(feature = "tracing-recover")]
                    warn!(field = ?f, "dropping IMF field with a body to be discarded");
                    continue;
                }
                Err(imf::field::InvalidField::Body) => {
                    // this is an IMF field but its body is invalid; drop it.
                    #[cfg(feature = "tracing-discard")]
                    warn!(field = ?f, "dropping IMF field with an invalid body");
                    continue;
                }
                Err(imf::field::InvalidField::Name) => {
                    // not an IMF field
                    ()
                }
            }

            if let Some(u) = header::Unstructured::from_raw(&f) {
                entries.push(MessageEntry::Unstructured(u));
            } else {
                // otherwise drop the field
                #[cfg(feature = "tracing-discard")]
                warn!(field = ?f, "dropping field that cannot be parsed as unstructured")
            }
        }
        entries.extend(
            imf.missing_mandatory_fields()
               .into_iter()
               .map(MessageEntry::Imf)
        );

        MessageFields {
            mime,
            imf: imf.to_imf(),
            entries,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::imf::datetime::DateTime;
    use crate::imf::{Imf, From};
    use crate::imf::address::*;
    use crate::imf::mailbox::*;
    use crate::mime::{CommonMIME, MIME};
    use crate::part::composite::Multipart;
    use crate::part::discrete::Text;
    use crate::part::{AnyPart, MimeBody};
    use crate::part::field::EntityEntry;
    use crate::print::tests::print_to_vec;
    use crate::text::charset::EmailCharset;
    use crate::text::encoding::{Base64Word, EncodedWord, EncodedWordToken, QuotedChunk, QuotedWord};
    use crate::text::misc_token::*;
    use crate::text::words::{Atom, MIMEAtom};
    use chrono::{FixedOffset, TimeZone};
    use pretty_assertions::assert_eq;

    fn test_message_roundtrip<'a>(txt: &[u8], parsed: Message<'a>) {
        assert_eq!(message(txt), parsed.clone());
        let printed = print_to_vec(parsed);
        assert_eq!(String::from_utf8_lossy(&printed), String::from_utf8_lossy(txt))
    }

    fn test_message_parse_print<'a>(txt: &[u8], parsed: Message<'a>, printed: &[u8]) {
        assert_eq!(message(txt), parsed.clone());
        let reprinted = print_to_vec(parsed);
        assert_eq!(String::from_utf8_lossy(&reprinted), String::from_utf8_lossy(printed))
    }

    fn test_message_reprint<'a>(txt: &[u8], printed: &[u8]) {
        let parsed = message(txt);
        let reprinted = print_to_vec(parsed);
        assert_eq!(String::from_utf8_lossy(&reprinted), String::from_utf8_lossy(printed))
    }

    #[test]
    fn test_simple() {
        let fullmail = b"Date: Tue, 7 Mar 2023 08:00:00 +0200\r
From: someone@example.com\r
To: someone_else@example.com\r
Subject: An  RFC 822  formatted message\r
MIME-Version: 1.0\r
\r
This is the plain text body of the message. Note the blank line
between the header information and the body of the message.";

        test_message_roundtrip(
            fullmail,
            {
                let from = MailboxRef {
                    name: None,
                    addrspec: AddrSpec {
                        local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(Atom("someone"[..].into())))]),
                        domain: Domain::Atoms(vec![Atom("example"[..].into()), Atom("com"[..].into())]),
                    }
                };
                let mut imf = Imf::new(
                    From::Single { from, sender: None },
                    DateTime(FixedOffset::east_opt(2 * 3600).unwrap().with_ymd_and_hms(2023, 3, 7, 8, 0, 0).unwrap()),
                );
                imf.to = vec![AddressRef::Single(MailboxRef {
                    name: None,
                    addrspec: AddrSpec {
                        local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(Atom("someone_else"[..].into())))]),
                        domain: Domain::Atoms(vec![Atom("example"[..].into()), Atom("com"[..].into())]),
                    }
                })];
                imf.subject = Some(Unstructured(vec![
                    UnstrToken::from_plain(" ", UnstrTxtKind::Fws),
                    UnstrToken::from_plain("An", UnstrTxtKind::Txt),
                    UnstrToken::from_plain("  ", UnstrTxtKind::Fws),
                    UnstrToken::from_plain("RFC", UnstrTxtKind::Txt),
                    UnstrToken::from_plain(" ", UnstrTxtKind::Fws),
                    UnstrToken::from_plain("822", UnstrTxtKind::Txt),
                    UnstrToken::from_plain("  ", UnstrTxtKind::Fws),
                    UnstrToken::from_plain("formatted", UnstrTxtKind::Txt),
                    UnstrToken::from_plain(" ", UnstrTxtKind::Fws),
                    UnstrToken::from_plain("message", UnstrTxtKind::Txt),
                ]));

                let mime_body = part::MimeBody::Txt(
                    part::discrete::Text {
                        mime: MIME {
                            ctype: mime::r#type::Text::default(),
                            fields: CommonMIME::default(),
                        },
                        body: b"This is the plain text body of the message. Note the blank line\nbetween the header information and the body of the message."[..].into(),
                    }
                );

                let entries = vec![
                    MessageEntry::Imf(imf::field::Entry::Date),
                    MessageEntry::Imf(imf::field::Entry::From),
                    MessageEntry::Imf(imf::field::Entry::To),
                    MessageEntry::Imf(imf::field::Entry::Subject),
                    MessageEntry::Imf(imf::field::Entry::MIMEVersion),
                ];

                Message {
                    imf,
                    mime_body,
                    entries,
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
Subject: Bad_redundant_subject
Content-Type: multipart/alternative;
 boundary="b1_e376dc71bafc953c0b0fdeb9983a9956"
Content-Transfer-Encoding: 7bit
Content-Transfer-Encoding: bad_redundant

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
                                    PhraseToken::Word(Word::Atom(Atom("Grrrnd"[..].into()))),
                                    PhraseToken::Word(Word::Atom(Atom("Zero"[..].into()))),
                                ])),
                                addrspec: imf::mailbox::AddrSpec {
                                    local_part: imf::mailbox::LocalPart(vec![
                                        imf::mailbox::LocalPartToken::Word(Word::Atom(Atom("grrrndzero"[..].into())))
                                    ]),
                                    domain: imf::mailbox::Domain::Atoms(vec![
                                        Atom("example"[..].into()),
                                        Atom("org"[..].into()),
                                    ]),
                                }
                            };
                        let date = imf::datetime::DateTime(FixedOffset::east_opt(2 * 3600)
                            .unwrap()
                            .with_ymd_and_hms(2023, 07, 8, 7, 14, 29)
                            .unwrap());

                        let mut imf = imf::Imf::new(imf::From::Single { from, sender: None }, date);

                        imf.to = vec![imf::address::AddressRef::Single(imf::mailbox::MailboxRef {
                                name: Some(Phrase(vec![
                                    PhraseToken::Word(Word::Atom(Atom("John"[..].into()))),
                                    PhraseToken::Word(Word::Atom(Atom("Doe"[..].into()))),
                                ])),
                                addrspec: imf::mailbox::AddrSpec {
                                    local_part: imf::mailbox::LocalPart(vec![
                                        imf::mailbox::LocalPartToken::Word(Word::Atom(Atom("jdoe"[..].into())))
                                    ]),
                                    domain: imf::mailbox::Domain::Atoms(vec![
                                        Atom("machine"[..].into()),
                                        Atom("example"[..].into()),
                                    ]),
                                }
                         })];

                        imf.cc = vec![imf::address::AddressRef::Single(imf::mailbox::MailboxRef {
                            name: Some(Phrase(vec![
                                PhraseToken::Encoded(EncodedWord(vec![
                                    EncodedWordToken::Quoted(QuotedWord {
                                        enc: EmailCharset::from(b"iso-8859-1"),
                                        chunks: vec![
                                            QuotedChunk::Safe(b"Andr"[..].into()),
                                            QuotedChunk::Encoded(vec![0xE9]),
                                        ],
                                    })
                                ])),
                                PhraseToken::Word(Word::Atom(Atom("Pirard"[..].into()))),
                            ])),
                            addrspec: imf::mailbox::AddrSpec {
                                local_part: imf::mailbox::LocalPart(vec![
                                    imf::mailbox::LocalPartToken::Word(Word::Atom(Atom("PIRARD"[..].into())))
                                ]),
                                domain: imf::mailbox::Domain::Atoms(vec![
                                    Atom("vm1"[..].into()),
                                    Atom("ulg"[..].into()),
                                    Atom("ac"[..].into()),
                                    Atom("be"[..].into()),
                                ]),
                            }
                        })];

                        imf.subject = Some(Unstructured(vec![
                            UnstrToken::from_plain(" ", UnstrTxtKind::Fws),
                            UnstrToken::Encoded(EncodedWord(vec![
                                EncodedWordToken::Base64(Base64Word{
                                    enc: EmailCharset::from(b"iso-8859-1"),
                                    content: b"SWYgeW91IGNhbiByZWFkIHRoaXMgeW8"[..].into(),
                                }),
                                EncodedWordToken::Base64(Base64Word{
                                    enc: EmailCharset::from(b"iso-8859-2"),
                                    content: b"dSB1bmRlcnN0YW5kIHRoZSBleGFtcGxlLg"[..].into(),
                                })
                            ])),
                        ]));

                        imf.msg_id = Some(imf::identification::MessageID::ObsLeftRight {
                            left: LocalPart(vec![
                                LocalPartToken::Word(Word::Atom(Atom("NTAxNzA2AC47634Y366BAMTY4ODc5MzQyODY0ODY5".into()))),
                            ]),
                            right: Domain::Atoms(vec![
                                Atom("www".into()),
                                Atom("grrrndzero".into()),
                                Atom("org".into()),
                            ]),
                        });

                        imf.discarded.push(imf::field::Field::Subject(Unstructured(vec![
                            UnstrToken::from_plain(" ", UnstrTxtKind::Fws),
                            UnstrToken::from_plain("Bad_redundant_subject", UnstrTxtKind::Txt),
                        ])));

                        imf
                    },
                    entries: vec![
                        MessageEntry::Imf(imf::field::Entry::Date),
                        MessageEntry::Imf(imf::field::Entry::From),
                        MessageEntry::Imf(imf::field::Entry::To),
                        MessageEntry::Imf(imf::field::Entry::Cc),
                        MessageEntry::Imf(imf::field::Entry::Subject),
                        MessageEntry::Unstructured(
                            header::Unstructured {
                                name: header::FieldName(b"X-Unknown"[..].into()),
                                body: Unstructured(vec![
                                    UnstrToken::from_plain(" ", UnstrTxtKind::Fws),
                                    UnstrToken::from_plain("something", UnstrTxtKind::Txt),
                                    UnstrToken::from_plain(" ", UnstrTxtKind::Fws),
                                    UnstrToken::from_plain("something", UnstrTxtKind::Txt),
                                ]),
                            }
                        ),
                        MessageEntry::Imf(imf::field::Entry::MessageId),
                        MessageEntry::Imf(imf::field::Entry::MIMEVersion),
                        MessageEntry::MIME(mime::field::Entry::Type),
                        MessageEntry::MIME(mime::field::Entry::TransferEncoding),
                    ],
                    mime_body: MimeBody::Mult(Multipart {
                        mime: mime::MIME {
                            ctype: mime::r#type::Multipart {
                                subtype: mime::r#type::MultipartSubtype::Alternative,
                                boundary: Some("b1_e376dc71bafc953c0b0fdeb9983a9956".to_string()),
                                params: vec![],
                            },
                            fields: mime::CommonMIME {
                                transfer_encoding: mime::mechanism::Mechanism::_7Bit,
                                discarded: vec![
                                    mime::field::Content::TransferEncoding(
                                        mime::mechanism::Mechanism::Other(MIMEAtom(b"bad_redundant".into()))
                                    ),
                                ],
                                ..mime::CommonMIME::default()
                            },
                        },
                        preamble: preamble.into(),
                        epilogue: vec![].into(),
                        children: vec![
                            AnyPart {
                                entries: vec![
                                    EntityEntry::MIME(mime::field::Entry::Type),
                                    EntityEntry::MIME(mime::field::Entry::TransferEncoding),
                                ],
                                mime_body: MimeBody::Txt(Text {
                                    mime: mime::MIME {
                                        ctype: mime::r#type::Text {
                                            subtype: mime::r#type::TextSubtype::Plain,
                                            charset: EmailCharset::utf8(),
                                            params: vec![],
                                        },
                                        fields: mime::CommonMIME {
                                            transfer_encoding: mime::mechanism::Mechanism::QuotedPrintable,
                                            ..mime::CommonMIME::default()
                                        }
                                    },
                                    body: b"GZ\nOoOoO\noOoOoOoOo\noOoOoOoOoOoOoOoOo\noOoOoOoOoOoOoOoOoOoOoOo\noOoOoOoOoOoOoOoOoOoOoOoOoOoOo\nOoOoOoOoOoOoOoOoOoOoOoOoOoOoOoOoO\n"[..].into(),
                                }),
                            },
                            AnyPart {
                                entries: vec![
                                    EntityEntry::Unstructured(header::Unstructured {
                                        name: header::FieldName(b"X-Custom".into()),
                                        body: Unstructured(vec![
                                            UnstrToken::from_plain(" ", UnstrTxtKind::Fws),
                                            UnstrToken::from_plain("foobar", UnstrTxtKind::Txt),
                                        ]),
                                    }),
                                    EntityEntry::MIME(mime::field::Entry::Type),
                                ],
                                mime_body: MimeBody::Txt(Text {
                                    mime: mime::MIME {
                                        ctype: mime::r#type::Text {
                                            subtype: mime::r#type::TextSubtype::Html,
                                            charset: EmailCharset::US_ASCII,
                                            params: vec![],
                                        },

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
                    }),
                };

        let reprinted: &[u8] = "Date: Sat, 8 Jul 2023 07:14:29 +0200\r
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
Content-Type: text/html; charset=us-ascii\r
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

    #[test]
    fn test_best_effort() {
        test_message_parse_print(
            b"date: uhh
hello: yolo

hello??",
            {
                let from = imf::mailbox::MailboxRef::placeholder();
                let imf = Imf::new(
                    From::Single { from, sender: None },
                    imf::datetime::DateTime::placeholder(),
                );

                let mime_body = part::MimeBody::Txt(
                    part::discrete::Text {
                        mime: MIME {
                            ctype: mime::r#type::Text::default(),
                            fields: CommonMIME::default(),
                        },
                        body: b"hello??"[..].into(),
                    }
                );

                let entries = vec![
                    MessageEntry::Unstructured(header::Unstructured {
                        name: header::FieldName(b"hello".into()),
                        body: Unstructured(vec![
                            UnstrToken::from_plain(" ", UnstrTxtKind::Fws),
                            UnstrToken::from_plain("yolo", UnstrTxtKind::Txt),
                        ]),
                    }),
                    MessageEntry::Imf(imf::field::Entry::Date),
                    MessageEntry::Imf(imf::field::Entry::From),
                    MessageEntry::Imf(imf::field::Entry::MIMEVersion),
                ];

                Message {
                    imf,
                    mime_body,
                    entries,
                }
            },
            b"hello: yolo\r
Date: Thu, 1 Jan 1970 00:00:00 +0000\r
From: unknown@unknown\r
MIME-Version: 1.0\r
\r
hello??",
        );
    }

    #[test]
    fn test_trace_unstructured() {
        test_message_reprint(
            b"X-Mozilla-Status: 0001
X-Mozilla-Status2: 00000000
Return-Path: <hello@sympa.lmf.cnrs.fr>
Received: from mx.lmf.cnrs.fr ([127.0.0.1])
        by mx.lmf.cnrs.fr with LMTP
        id oFAUKCuwpWmTPRAAFSOJEQ
        (envelope-from <infos-gs-owner@sympa.lmf.cnrs.fr>); Mon, 02 Mar 2026 15:43:39 +0000
X-Spam-Checker-Version: SpamAssassin 3.4.6 (2021-04-09) on mx.lmf.cnrs.fr
Received-SPF: Pass (mailfrom) identity=mailfrom; client-ip=10.0.0.2; helo=sympa.lmf.cnrs.fr; envelope-from=hello@sympa.lmf.cnrs.fr; receiver=<UNKNOWN>
Received: from sympa.lmf.cnrs.fr (sympa.lmf.cnrs.fr [10.0.0.2])
        (using TLSv1.3 with cipher TLS_AES_256_GCM_SHA384 (256/256 bits)
         key-exchange X25519 server-signature RSA-PSS (2048 bits))
        (No client certificate requested)
        by mx.lmf.cnrs.fr (Postfix) with ESMTPS id DC88D214EA;
        Mon,  2 Mar 2026 15:43:37 +0000 (UTC)
Received: by sympa.lmf.cnrs.fr (Postfix, from userid 106)
        id ACE8B4A03ED; Mon,  2 Mar 2026 16:43:37 +0100 (CET)
",
            b"X-Mozilla-Status: 0001\r
X-Mozilla-Status2: 00000000\r
Return-Path: <hello@sympa.lmf.cnrs.fr>\r
Received: from mx.lmf.cnrs.fr ([127.0.0.1])        by mx.lmf.cnrs.fr with LMTP\r
        id oFAUKCuwpWmTPRAAFSOJEQ        (envelope-from\r
 <infos-gs-owner@sympa.lmf.cnrs.fr>); Mon, 02 Mar 2026 15:43:39 +0000\r
X-Spam-Checker-Version: SpamAssassin 3.4.6 (2021-04-09) on mx.lmf.cnrs.fr\r
Received-SPF: Pass (mailfrom) identity=mailfrom; client-ip=10.0.0.2;\r
 helo=sympa.lmf.cnrs.fr; envelope-from=hello@sympa.lmf.cnrs.fr;\r
 receiver=<UNKNOWN>\r
Received: from sympa.lmf.cnrs.fr (sympa.lmf.cnrs.fr [10.0.0.2])        (using\r
 TLSv1.3 with cipher TLS_AES_256_GCM_SHA384 (256/256 bits)        \r
 key-exchange X25519 server-signature RSA-PSS (2048 bits))        (No client\r
 certificate requested)        by mx.lmf.cnrs.fr (Postfix) with ESMTPS id\r
 DC88D214EA;        Mon,  2 Mar 2026 15:43:37 +0000 (UTC)\r
Received: by sympa.lmf.cnrs.fr (Postfix, from userid 106)        id\r
 ACE8B4A03ED; Mon,  2 Mar 2026 16:43:37 +0100 (CET)\r
Date: Thu, 1 Jan 1970 00:00:00 +0000\r
From: unknown@unknown\r
MIME-Version: 1.0\r
\r
"
        );
    }

    // tests for UTF8 from https://github.com/arnt/eai-test-messages

    #[test]
    fn test_utf8_addresses() {
        test_message_reprint(
            "From: Jøran Øygårdvær <jøran@example.com>
Cc: Jøran Øygårdvær <jøran@example.com>
Signed-Off-By: Jøran Øygårdvær <jøran@example.com>
To: Arnt Gulbrandsen <arnt@example.com>
Date: Thu, 20 May 2004 14:28:51 +0200

".as_bytes(),

            "From: Jøran Øygårdvær <jøran@example.com>\r
Cc: Jøran Øygårdvær <jøran@example.com>\r
Signed-Off-By: Jøran Øygårdvær <jøran@example.com>\r
To: Arnt Gulbrandsen <arnt@example.com>\r
Date: Thu, 20 May 2004 14:28:51 +0200\r
MIME-Version: 1.0\r
\r
".as_bytes()
        );
    }

    #[test]
    fn test_utf8_attachment() {
        test_message_reprint(
            r#"From: Arnt Gulbrandsen <arnt@example.com>
To: Arnt Gulbrandsen <arnt@example.com>
Date: Thu, 20 May 2004 14:28:51 +0200
Content-Type: multipart/mixed; boundary=-
Mime-Version: 1.0

---
Content-Type: text/plain; format=flowed; x-eai-please-do-not="abstürzen"

There's nothing to do about this bodypart, except not crash. The attachment
has a somewhat challenging filename.

---
Content-Disposition: attachment; filename="blåbærsyltetøy"
Content-Type: image/jpeg
Content-Transfer-Encoding: base64

snip
-----
"#.as_bytes(),

            "From: Arnt Gulbrandsen <arnt@example.com>\r
To: Arnt Gulbrandsen <arnt@example.com>\r
Date: Thu, 20 May 2004 14:28:51 +0200\r
Content-Type: multipart/mixed;\r
 boundary=\"V1Qy0rpB5tWE76WF3UelfGW5K9LZpjHjZ3PKE26vpVNnvofq7BLuYTWxzQB3HrYu7\"\r
MIME-Version: 1.0\r
\r
--V1Qy0rpB5tWE76WF3UelfGW5K9LZpjHjZ3PKE26vpVNnvofq7BLuYTWxzQB3HrYu7\r
Content-Type: text/plain; charset=us-ascii; format=flowed;\r
 x-eai-please-do-not=\"abstürzen\"\r
\r
There's nothing to do about this bodypart, except not crash. The attachment
has a somewhat challenging filename.
\r
--V1Qy0rpB5tWE76WF3UelfGW5K9LZpjHjZ3PKE26vpVNnvofq7BLuYTWxzQB3HrYu7\r
Content-Disposition: attachment; filename=\"blåbærsyltetøy\"\r
Content-Type: image/jpeg\r
Content-Transfer-Encoding: base64\r
\r
snip\r
--V1Qy0rpB5tWE76WF3UelfGW5K9LZpjHjZ3PKE26vpVNnvofq7BLuYTWxzQB3HrYu7--\r
".as_bytes()
        );
    }

    #[test]
    fn test_utf8_from() {
        test_message_reprint(
            "From: Jøran Øygårdvær <jøran@example.com>
To: Arnt Gulbrandsen <arnt@example.com>
Date: Thu, 20 May 2004 14:28:51 +0200

asdf".as_bytes(),
            "From: Jøran Øygårdvær <jøran@example.com>\r
To: Arnt Gulbrandsen <arnt@example.com>\r
Date: Thu, 20 May 2004 14:28:51 +0200\r
MIME-Version: 1.0\r
\r
asdf".as_bytes(),
        );
    }

    #[test]
    fn test_utf8_mimefield() {
        test_message_reprint(
            "From: Arnt Gulbrandsen <arnt@example.com>\r
To: Arnt Gulbrandsen <arnt@example.com>\r
Date: Thu, 20 May 2004 14:28:51 +0200\r
Content-Disposition: attachment; filename=\"blåbærsyltetøy\"\r
Content-Type: text/plain; format=flowed\r
Mime-Version: 1.0\r
\r
It's a bit odd that a single-part message is an attachment with a
filename. But perfectly legal.".as_bytes(),

            "From: Arnt Gulbrandsen <arnt@example.com>\r
To: Arnt Gulbrandsen <arnt@example.com>\r
Date: Thu, 20 May 2004 14:28:51 +0200\r
Content-Disposition: attachment; filename=\"blåbærsyltetøy\"\r
Content-Type: text/plain; charset=us-ascii; format=flowed\r
MIME-Version: 1.0\r
\r
It's a bit odd that a single-part message is an attachment with a
filename. But perfectly legal.".as_bytes()
        );
    }

    #[test]
    fn test_message_global_recover() {
        // If an embedded message contains UTF8, ensure its content type is
        // message/global. (message/rfc822 is not supposed to contain UTF-8
        // headers but we parse those nevertheless...)
        test_message_reprint(
            "From: admin@example.com
To: user@example.com
Date: Thu, 20 May 2004 14:28:51 +0200
Content-Type: message/rfc822

From: \"Armaël\" <armaël@example.com>
To: \"Müller\" <müller@example.test>
Subject: Café? ☕
Content-Type: text/plain; charset=\"utf-8\"

☕?".as_bytes(),

            "From: admin@example.com\r
To: user@example.com\r
Date: Thu, 20 May 2004 14:28:51 +0200\r
Content-Type: message/global\r
MIME-Version: 1.0\r
\r
From: \"Armaël\" <armaël@example.com>\r
To: \"Müller\" <müller@example.test>\r
Subject: Café? ☕\r
Content-Type: text/plain; charset=UTF-8\r
\r
☕?".as_bytes()
        );
    }
}
