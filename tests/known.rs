use chrono::{FixedOffset, TimeZone};
use imf_codec::fragments::{misc_token, model, section, part, trace};
use imf_codec::multipass;
use std::collections::HashMap;

fn parser<'a, F>(input: &'a [u8], func: F) -> ()
where
    F: FnOnce(&section::Section) -> (),
{
    let seg = multipass::segment::new(input).unwrap();
    let charset = seg.charset();
    let fields = charset.fields().unwrap();
    let field_names = fields.names();
    let field_body = field_names.body();
    let section = field_body.section();

    func(&section.fields);
}

#[test]
fn test_headers() {
    let fullmail: &[u8] = r#"Return-Path: <gitlab@example.com>
Delivered-To: quentin@example.com
Received: from smtp.example.com ([10.83.2.2])
	by doradille with LMTP
	id xyzabcd
	(envelope-from <gitlab@example.com>)
	for <quentin@example.com>; Tue, 13 Jun 2023 19:01:08 +0000
Date: Tue, 13 Jun 2023 10:01:10 +0200
From: Mary Smith
 <mary@example.net>, "A\lan" <alan@example>
Sender: imf@example.com
Reply-To: "Mary Smith: Personal Account" <smith@home.example>
To: John Doe <jdoe@machine.example>
Cc: imf2@example.com
Bcc: (hidden)
Subject: Re: Saying Hello
Comments: A simple message
Comments: Not that complicated
comments : not valid header name but should be accepted
    by the parser.
Keywords: hello, world
Héron: Raté
 Raté raté
Keywords: salut, le, monde
Not a real header but should still recover
Message-ID: <3456@example.net>
In-Reply-To: <1234@local.machine.example>
References: <1234@local.machine.example>
Unknown: unknown

This is a reply to your hello.
"#
    .as_bytes();
    parser(fullmail, |parsed_section| {
        assert_eq!(
            parsed_section,
            &section::Section {
                date: Some(
                    &FixedOffset::east_opt(2 * 3600)
                        .unwrap()
                        .with_ymd_and_hms(2023, 06, 13, 10, 01, 10)
                        .unwrap()
                ),

                from: vec![
                    &model::MailboxRef {
                        name: Some("Mary Smith".into()),
                        addrspec: model::AddrSpec {
                            local_part: "mary".into(),
                            domain: "example.net".into(),
                        }
                    },
                    &model::MailboxRef {
                        name: Some("Alan".into()),
                        addrspec: model::AddrSpec {
                            local_part: "alan".into(),
                            domain: "example".into(),
                        }
                    }
                ],

                sender: Some(&model::MailboxRef {
                    name: None,
                    addrspec: model::AddrSpec {
                        local_part: "imf".into(),
                        domain: "example.com".into(),
                    }
                }),

                reply_to: vec![&model::AddressRef::Single(model::MailboxRef {
                    name: Some("Mary Smith: Personal Account".into()),
                    addrspec: model::AddrSpec {
                        local_part: "smith".into(),
                        domain: "home.example".into(),
                    }
                })],

                to: vec![&model::AddressRef::Single(model::MailboxRef {
                    name: Some("John Doe".into()),
                    addrspec: model::AddrSpec {
                        local_part: "jdoe".into(),
                        domain: "machine.example".into(),
                    }
                })],

                cc: vec![&model::AddressRef::Single(model::MailboxRef {
                    name: None,
                    addrspec: model::AddrSpec {
                        local_part: "imf2".into(),
                        domain: "example.com".into(),
                    }
                })],

                bcc: vec![],

                msg_id: Some(&model::MessageId {
                    left: "3456",
                    right: "example.net"
                }),
                in_reply_to: vec![&model::MessageId {
                    left: "1234",
                    right: "local.machine.example"
                }],
                references: vec![&model::MessageId {
                    left: "1234",
                    right: "local.machine.example"
                }],

                subject: Some(&misc_token::Unstructured("Re: Saying Hello".into())),

                comments: vec![
                    &misc_token::Unstructured("A simple message".into()),
                    &misc_token::Unstructured("Not that complicated".into()),
                    &misc_token::Unstructured(
                        "not valid header name but should be accepted by the parser.".into()
                    ),
                ],

                keywords: vec![
                    &misc_token::PhraseList(vec!["hello".into(), "world".into(),]),
                    &misc_token::PhraseList(vec!["salut".into(), "le".into(), "monde".into(),]),
                ],

                received: vec![&trace::ReceivedLog(
                    r#"from smtp.example.com ([10.83.2.2])
	by doradille with LMTP
	id xyzabcd
	(envelope-from <gitlab@example.com>)
	for <quentin@example.com>"#
                )],

                return_path: vec![&model::MailboxRef {
                    name: None,
                    addrspec: model::AddrSpec {
                        local_part: "gitlab".into(),
                        domain: "example.com".into(),
                    }
                }],

                optional: HashMap::from([
                    (
                        "Delivered-To",
                        &misc_token::Unstructured("quentin@example.com".into())
                    ),
                    ("Unknown", &misc_token::Unstructured("unknown".into())),
                ]),

                bad_fields: vec![],

                unparsed: vec![
                    "Héron: Raté\n Raté raté\n",
                    "Not a real header but should still recover\n",
                ],
                ..section::Section::default()
            }
        )
    })
}

#[test]
fn test_headers_mime() {
    use imf_codec::fragments::mime;
    let fullmail: &[u8] = r#"From: =?US-ASCII?Q?Keith_Moore?= <moore@cs.utk.edu>
To: =?ISO-8859-1?Q?Keld_J=F8rn_Simonsen?= <keld@dkuug.dk>
CC: =?ISO-8859-1?Q?Andr=E9?= Pirard <PIRARD@vm1.ulg.ac.be>
Subject: =?ISO-8859-1?B?SWYgeW91IGNhbiByZWFkIHRoaXMgeW8=?=
    =?ISO-8859-2?B?dSB1bmRlcnN0YW5kIHRoZSBleGFtcGxlLg==?=
MIME-Version: 1.0
Content-Type: text/plain; charset=ISO-8859-1
Content-Transfer-Encoding: quoted-printable
Content-ID: <a@example.com>
Content-Description: hello

Now's the time =
for all folk to come=
 to the aid of their country.
"#
    .as_bytes();

   parser(fullmail, |parsed_section| {
        assert_eq!(
            parsed_section,
            &section::Section {
                from: vec![
                    &model::MailboxRef {
                        name: Some("Keith Moore".into()),
                        addrspec: model::AddrSpec {
                            local_part: "moore".into(),
                            domain: "cs.utk.edu".into(),
                        }
                    },
                ],

                to: vec![&model::AddressRef::Single(model::MailboxRef {
                    name: Some("Keld Jørn Simonsen".into()),
                    addrspec: model::AddrSpec {
                        local_part: "keld".into(),
                        domain: "dkuug.dk".into(),
                    }
                })],

                cc: vec![&model::AddressRef::Single(model::MailboxRef {
                    name: Some("André Pirard".into()),
                    addrspec: model::AddrSpec {
                        local_part: "PIRARD".into(),
                        domain: "vm1.ulg.ac.be".into(),
                    }
                })],

                subject: Some(&misc_token::Unstructured("If you can read this you understand the example.".into())),
                mime_version: Some(&mime::Version{ major: 1, minor: 0 }),
                content_type: Some(&mime::Type::Text(mime::TextDesc { 
                    charset: Some(mime::EmailCharset::ISO_8859_1), 
                    subtype: mime::TextSubtype::Plain, 
                    unknown_parameters: vec![]
                })),
                content_transfer_encoding: Some(&mime::Mechanism::QuotedPrintable),
                content_id: Some(&model::MessageId {
                    left: "a",
                    right: "example.com"
                }),
                content_description: Some(&misc_token::Unstructured("hello".into())),
                ..section::Section::default()
            }
        );
   })
}

fn parser_bodystruct<'a, F>(input: &'a [u8], func: F) -> ()
where
    F: FnOnce(&part::PartNode) -> (),
{
    let seg = multipass::segment::new(input).unwrap();
    let charset = seg.charset();
    let fields = charset.fields().unwrap();
    let field_names = fields.names();
    let field_body = field_names.body();
    let section = field_body.section();
    let bodystruct = section.body_structure();

    func(&bodystruct.body);
}

#[test]
fn test_multipart() {
    let fullmail: &[u8] = r#"Date: Sat, 8 Jul 2023 07:14:29 +0200
From: Grrrnd Zero <grrrndzero@example.org>
To: John Doe <jdoe@machine.example>
Subject: Re: Saying Hello
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

--b1_e376dc71bafc953c0b0fdeb9983a9956--
"#.as_bytes();
    
    parser_bodystruct(fullmail, |part| {
        assert_eq!(part, &part::PartNode::Composite(
            part::PartHeader {
                ..part::PartHeader::default()
            },
            vec![
                part::PartNode::Discrete(
                    part::PartHeader {
                        ..part::PartHeader::default()
                    },
                    r#"GZ
OoOoO
oOoOoOoOo
oOoOoOoOoOoOoOoOo
oOoOoOoOoOoOoOoOoOoOoOo
oOoOoOoOoOoOoOoOoOoOoOoOoOoOo
OoOoOoOoOoOoOoOoOoOoOoOoOoOoOoOoO"#.as_bytes()
                ),
                part::PartNode::Discrete(
                    part::PartHeader {
                        ..part::PartHeader::default()
                    },
                    r#"<div style="text-align: center;"><strong>GZ</strong><br />
OoOoO<br />
oOoOoOoOo<br />
oOoOoOoOoOoOoOoOo<br />
oOoOoOoOoOoOoOoOoOoOoOo<br />
oOoOoOoOoOoOoOoOoOoOoOoOoOoOo<br />
OoOoOoOoOoOoOoOoOoOoOoOoOoOoOoOoO<br />"#.as_bytes()
                ),
            ]));
    });
}
