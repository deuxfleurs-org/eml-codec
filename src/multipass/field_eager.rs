use crate::fragments::eager;
use crate::multipass::field_lazy;
use crate::multipass::header_section;

#[derive(Debug, PartialEq)]
pub struct Parsed<'a> {
    pub fields: Vec<eager::Field<'a>>,
    pub body: &'a [u8],
}

pub fn new<'a>(p: &'a field_lazy::Parsed<'a>) -> Parsed<'a> {
    Parsed {
        fields: p
            .fields
            .iter()
            .filter_map(|entry| entry.try_into().ok())
            .collect(),
        body: p.body,
    }
}

impl<'a> Parsed<'a> {
    pub fn section(&'a self) -> header_section::Parsed<'a> {
        header_section::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fragments::lazy;
    use crate::fragments::model;
    use chrono::{FixedOffset, TimeZone};

    #[test]
    fn test_field_body() {
        assert_eq!(
            new(&field_lazy::Parsed {
                fields: vec![
                    lazy::Field::From(lazy::MailboxList(
                        "hello@world.com,\r\n\talice@wonderlands.com\r\n"
                    )),
                    lazy::Field::Date(lazy::DateTime("12 Mar 1997 07:33:25 Z\r\n")),
                ],
                body: b"Hello world!",
            }),
            Parsed {
                fields: vec![
                    eager::Field::From(vec![
                        model::MailboxRef {
                            name: None,
                            addrspec: model::AddrSpec {
                                local_part: "hello".into(),
                                domain: "world.com".into()
                            }
                        },
                        model::MailboxRef {
                            name: None,
                            addrspec: model::AddrSpec {
                                local_part: "alice".into(),
                                domain: "wonderlands.com".into()
                            }
                        },
                    ]),
                    eager::Field::Date(
                        FixedOffset::east_opt(0)
                            .unwrap()
                            .with_ymd_and_hms(1997, 03, 12, 7, 33, 25)
                            .unwrap()
                    ),
                ],
                body: b"Hello world!",
            }
        );
    }

    use crate::fragments::misc_token;
    use crate::multipass::extract_fields;
    fn lazy_eager<F>(input: &str, func: F)
    where
        F: Fn(&eager::Field),
    {
        let field = extract_fields::Parsed {
            fields: vec![input],
            body: b"",
        };
        let lazy = field_lazy::new(&field);
        let eager = new(&lazy);
        func(eager.fields.first().unwrap())
    }

    #[test]
    fn test_from() {
        lazy_eager(
            "From: \"Joe Q. Public\" <john.q.public@example.com>\r\n",
            |from| {
                assert_eq!(
                    from,
                    &eager::Field::From(vec![model::MailboxRef {
                        name: Some("Joe Q. Public".into()),
                        addrspec: model::AddrSpec {
                            local_part: "john.q.public".into(),
                            domain: "example.com".into(),
                        }
                    }])
                )
            },
        );
    }

    #[test]
    fn test_sender() {
        lazy_eager(
            "Sender: Michael Jones <mjones@machine.example>\r\n",
            |sender| {
                assert_eq!(
                    sender,
                    &eager::Field::Sender(model::MailboxRef {
                        name: Some("Michael Jones".into()),
                        addrspec: model::AddrSpec {
                            local_part: "mjones".into(),
                            domain: "machine.example".into(),
                        },
                    })
                )
            },
        );
    }

    #[test]
    fn test_reply_to() {
        lazy_eager(
            "Reply-To: \"Mary Smith: Personal Account\" <smith@home.example>\r\n",
            |reply_to| {
                assert_eq!(
                    reply_to,
                    &eager::Field::ReplyTo(vec![model::AddressRef::Single(model::MailboxRef {
                        name: Some("Mary Smith: Personal Account".into()),
                        addrspec: model::AddrSpec {
                            local_part: "smith".into(),
                            domain: "home.example".into(),
                        },
                    })])
                )
            },
        )
    }

    #[test]
    fn test_to() {
        lazy_eager(
            "To: A Group:Ed Jones <c@a.test>,joe@where.test,John <jdoe@one.test>;\r\n",
            |to| {
                assert_eq!(
                    to,
                    &eager::Field::To(vec![model::AddressRef::Many(model::GroupRef {
                        name: "A Group".into(),
                        participants: vec![
                            model::MailboxRef {
                                name: Some("Ed Jones".into()),
                                addrspec: model::AddrSpec {
                                    local_part: "c".into(),
                                    domain: "a.test".into()
                                },
                            },
                            model::MailboxRef {
                                name: None,
                                addrspec: model::AddrSpec {
                                    local_part: "joe".into(),
                                    domain: "where.test".into()
                                },
                            },
                            model::MailboxRef {
                                name: Some("John".into()),
                                addrspec: model::AddrSpec {
                                    local_part: "jdoe".into(),
                                    domain: "one.test".into()
                                },
                            },
                        ]
                    })])
                )
            },
        )
    }

    #[test]
    fn test_cc() {
        lazy_eager("Cc: Undisclosed recipients:;\r\n", |cc| {
            assert_eq!(
                cc,
                &eager::Field::Cc(vec![model::AddressRef::Many(model::GroupRef {
                    name: "Undisclosed recipients".into(),
                    participants: vec![],
                })]),
            )
        })
    }

    #[test]
    fn test_bcc() {
        lazy_eager("Bcc: (empty)\r\n", |bcc| {
            assert_eq!(bcc, &eager::Field::Bcc(vec![]),)
        });

        lazy_eager("Bcc: \r\n", |bcc| {
            assert_eq!(bcc, &eager::Field::Bcc(vec![]),)
        });
    }

    #[test]
    fn test_message_id() {
        lazy_eager("Message-ID: <310@[127.0.0.1]>\r\n", |msg_id| {
            assert_eq!(
                msg_id,
                &eager::Field::MessageID(model::MessageId {
                    left: "310",
                    right: "127.0.0.1"
                },)
            )
        })
    }

    #[test]
    fn test_in_reply_to() {
        lazy_eager("In-Reply-To: <a@b> <c@example.com>\r\n", |irt| {
            assert_eq!(
                irt,
                &eager::Field::InReplyTo(vec![
                    model::MessageId {
                        left: "a",
                        right: "b"
                    },
                    model::MessageId {
                        left: "c",
                        right: "example.com"
                    },
                ])
            )
        })
    }

    #[test]
    fn test_references() {
        lazy_eager(
            "References: <1234@local.machine.example> <3456@example.net>\r\n",
            |refer| {
                assert_eq!(
                    refer,
                    &eager::Field::References(vec![
                        model::MessageId {
                            left: "1234",
                            right: "local.machine.example"
                        },
                        model::MessageId {
                            left: "3456",
                            right: "example.net"
                        },
                    ])
                )
            },
        )
    }

    #[test]
    fn test_subject() {
        lazy_eager("Subject: Aérogramme\r\n", |subject| {
            assert_eq!(
                subject,
                &eager::Field::Subject(misc_token::Unstructured("Aérogramme".into()))
            )
        })
    }

    #[test]
    fn test_comments() {
        lazy_eager("Comments: 😛 easter egg!\r\n", |comments| {
            assert_eq!(
                comments,
                &eager::Field::Comments(misc_token::Unstructured("😛 easter egg!".into())),
            )
        })
    }

    #[test]
    fn test_keywords() {
        lazy_eager(
            "Keywords: fantasque, farfelu, fanfreluche\r\n",
            |keywords| {
                assert_eq!(
                    keywords,
                    &eager::Field::Keywords(misc_token::PhraseList(vec![
                        "fantasque".into(),
                        "farfelu".into(),
                        "fanfreluche".into()
                    ]))
                )
            },
        )
    }

    //@FIXME non ported tests:

    /*
        #[test]
        fn test_invalid_field_name() {
            assert!(known_field("Unknown: unknown\r\n").is_err());
        }

        #[test]
        fn test_rescue_field() {
            assert_eq!(
                rescue_field("Héron: élan\r\n\tnoël: test\r\nFrom: ..."),
                Ok(("From: ...", Field::Rescue("Héron: élan\r\n\tnoël: test"))),
            );
        }

        #[test]
        fn test_wrong_fields() {
            let fullmail = r#"Return-Path: xoxo
    From: !!!!

    Hello world"#;
            assert_eq!(
                section(fullmail),
                Ok(("Hello world", HeaderSection {
                    bad_fields: vec![
                        Field::ReturnPath(FieldBody::Failed("xoxo")),
                        Field::From(FieldBody::Failed("!!!!")),
                    ],
                    ..Default::default()
                }))
            );
        }
        */
}
