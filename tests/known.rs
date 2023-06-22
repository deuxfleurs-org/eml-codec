use chrono::{FixedOffset, TimeZone};
use std::collections::HashMap;
use imf_codec::multipass;
use imf_codec::fragments::{model, misc_token, trace, section};

fn parser<'a, F>(input: &'a [u8], func: F) -> ()
where F: FnOnce(&section::Section) -> () {
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
"#.as_bytes();
    parser(fullmail, |parsed_section| 
        assert_eq!(
            parsed_section,
            &section::Section {
                date: Some(&FixedOffset::east_opt(2 * 3600)
                           .unwrap()
                           .with_ymd_and_hms(2023, 06, 13, 10, 01, 10)
                           .unwrap()),

                from: vec![&model::MailboxRef {
                    name: Some("Mary Smith".into()),
                    addrspec: model::AddrSpec {
                        local_part: "mary".into(),
                        domain: "example.net".into(),
                    }
                }, &model::MailboxRef {
                    name: Some("Alan".into()),
                    addrspec: model::AddrSpec {
                        local_part: "alan".into(),
                        domain: "example".into(),
                    }
                }],

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

                msg_id: Some(&model::MessageId { left: "3456", right: "example.net" }),
                in_reply_to: vec![&model::MessageId { left: "1234", right: "local.machine.example" }],
                references: vec![&model::MessageId { left: "1234", right: "local.machine.example" }],

                subject: Some(&misc_token::Unstructured("Re: Saying Hello".into())),

                comments: vec![
                    &misc_token::Unstructured("A simple message".into()),
                    &misc_token::Unstructured("Not that complicated".into()),
                    &misc_token::Unstructured("not valid header name but should be accepted by the parser.".into()),
                ],

                keywords: vec![
                    &misc_token::PhraseList(vec![
                        "hello".into(), 
                        "world".into(), 
                    ]),
                    &misc_token::PhraseList(vec![
                        "salut".into(), 
                        "le".into(), 
                        "monde".into(),
                    ]),
                ],

                received: vec![ 
                    &trace::ReceivedLog("from smtp.example.com ([10.83.2.2])\n\tby doradille with LMTP\n\tid xyzabcd\n\t(envelope-from <gitlab@example.com>)\n\tfor <quentin@example.com>")
                ],

                return_path: vec![&model::MailboxRef {
                    name: None,
                    addrspec: model::AddrSpec {
                        local_part: "gitlab".into(),
                        domain: "example.com".into(),
                    }
                }],

                optional: HashMap::from([
                    ("Delivered-To", &misc_token::Unstructured("quentin@example.com".into())),
                    ("Unknown", &misc_token::Unstructured("unknown".into())), 
                ]),

                bad_fields: vec![],

                unparsed: vec![
                    "Héron: Raté\n Raté raté\n",
                    "Not a real header but should still recover\n",
                ],
            }
        )
    )
}