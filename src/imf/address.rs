use bounded_static::ToStatic;
use nom::{
    branch::alt,
    bytes::complete::tag,
    combinator::{into, map, map_opt, opt},
    multi::separated_list1,
    sequence::tuple,
    IResult,
};

//use crate::error::IMFError;
use crate::print::{print_seq, Print, Formatter};
use crate::imf::mailbox::{mailbox, mailbox_list_nullable, MailboxRef, MailboxList};
use crate::text::misc_token::{phrase, Phrase};
use crate::text::whitespace::cfws;
use crate::utils::vec_filter_none_nonempty;

#[derive(Clone, Debug, PartialEq, ToStatic)]
pub struct GroupRef<'a> {
    pub name: Phrase<'a>,
    pub participants: Option<MailboxList<'a>>,
}
impl<'a> Print for GroupRef<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        self.name.print(fmt);
        fmt.write_bytes(b":");
        if let Some(mboxs) = &self.participants {
            mboxs.print(fmt);
        }
        fmt.write_bytes(b";")
    }
}

#[derive(Clone, Debug, PartialEq, ToStatic)]
pub enum AddressRef<'a> {
    Single(MailboxRef<'a>),
    Many(GroupRef<'a>),
}
impl<'a> From<MailboxRef<'a>> for AddressRef<'a> {
    fn from(mx: MailboxRef<'a>) -> Self {
        AddressRef::Single(mx)
    }
}
impl<'a> From<GroupRef<'a>> for AddressRef<'a> {
    fn from(grp: GroupRef<'a>) -> Self {
        AddressRef::Many(grp)
    }
}
impl<'a> Print for AddressRef<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        match self {
            AddressRef::Single(mbox) => mbox.print(fmt),
            AddressRef::Many(group) => group.print(fmt),
        }
    }
}

pub type AddressList<'a> = Vec<AddressRef<'a>>;

impl<'a> Print for AddressList<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        print_seq(fmt, self, |fmt| {
            fmt.write_bytes(b",");
            fmt.write_fws()
        })
    }
}

/// Address (section 3.4 of RFC5322)
///
/// ```abnf
///    address         =   mailbox / group
/// ```
pub fn address(input: &[u8]) -> IResult<&[u8], AddressRef<'_>> {
    alt((into(mailbox), into(group)))(input)
}

/// Group
///
/// ```abnf
///    group           =   display-name ":" [group-list] ";" [CFWS]
///    display-name    =   phrase
/// ```
pub fn group(input: &[u8]) -> IResult<&[u8], GroupRef<'_>> {
    let (input, (grp_name, _, grp_list, _, _)) =
        tuple((phrase, tag(":"), opt(group_list), tag(";"), opt(cfws)))(input)?;

    Ok((
        input,
        GroupRef {
            name: grp_name,
            participants: grp_list.unwrap_or(None),
        },
    ))
}

/// Group list
///
/// ```abnf
///    group-list      =   mailbox-list / CFWS / obs-group-list
///    obs-group-list  =   1*([CFWS] ",") [CFWS]
/// ```
pub fn group_list(input: &[u8]) -> IResult<&[u8], Option<MailboxList<'_>>> {
    mailbox_list_nullable(input)
}

/// Address list
///
/// ```abnf
///   address-list    =   (address *("," address)) / obs-addr-list
///   obs-addr-list   =   *([CFWS] ",") address *("," [address / CFWS])
/// ```
pub fn address_list(input: &[u8]) -> IResult<&[u8], Vec<AddressRef<'_>>> {
    map_opt(
        separated_list1(
            tag(","),
            alt((
                map(address, Some),
                map(opt(cfws), |_| None),
            ))
        ),
        vec_filter_none_nonempty
    )(input)
}

pub fn address_list_cfws(input: &[u8]) -> IResult<&[u8], Vec<AddressRef<'_>>> {
    let (input, _) = cfws(input)?;
    Ok((input, vec![]))
}

pub fn nullable_address_list(input: &[u8]) -> IResult<&[u8], Vec<AddressRef<'_>>> {
    map(opt(alt((address_list, address_list_cfws))), |v| {
        v.unwrap_or(vec![])
    })(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::charset::EmailCharset;
    use crate::imf::mailbox::{AddrSpec, Domain, LocalPart, LocalPartToken};
    use crate::print::tests::with_formatter;
    use crate::text::misc_token::{Phrase, PhraseToken, Word};
    use crate::text::words::Atom;

    fn address_list_parsed_printed(addrlist: &[u8], printed: &[u8], parsed: AddressList<'_>) {
        assert_eq!(address_list(addrlist).unwrap(), (&b""[..], parsed.clone()));
        let reprinted = with_formatter(|f| parsed.print(f));
        assert_eq!(String::from_utf8_lossy(&reprinted), String::from_utf8_lossy(printed));
    }

    fn address_list_reprinted(addrlist: &[u8], printed: &[u8]) {
        let (input, parsed) = address_list(addrlist).unwrap();
        assert!(input.is_empty());
        let reprinted = with_formatter(|f| parsed.print(f));
        assert_eq!(String::from_utf8_lossy(&reprinted), String::from_utf8_lossy(printed));
    }

    #[test]
    fn test_address_list() {
        address_list_parsed_printed(
            r#"A Group:Ed Jones <c@a.test>,joe@where.test,John <jdoe@one.test>;, Mary Smith <mary@x.test>"#.as_bytes(),
            r#"A Group:Ed Jones <c@a.test>, joe@where.test, John <jdoe@one.test>;, Mary Smith <mary@x.test>"#.as_bytes(),
            vec![
                AddressRef::Many(GroupRef {
                    name: Phrase(vec![
                        PhraseToken::Word(Word::Atom(Atom(b"A"[..].into()))),
                        PhraseToken::Word(Word::Atom(Atom(b"Group"[..].into()))),
                    ]),
                    participants: Some(MailboxList(vec![
                        MailboxRef {
                            name: Some(Phrase(vec![
                                PhraseToken::Word(Word::Atom(Atom(b"Ed"[..].into()))),
                                PhraseToken::Word(Word::Atom(Atom(b"Jones"[..].into()))),
                            ])),
                            addrspec: AddrSpec {
                                local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(Atom(b"c"[..].into())))]),
                                domain: Domain::Atoms(vec![Atom(b"a"[..].into()), Atom(b"test"[..].into())]),
                            },
                        },
                        MailboxRef {
                            name: None,
                            addrspec: AddrSpec {
                                local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(Atom(b"joe"[..].into())))]),
                                domain: Domain::Atoms(vec![Atom(b"where"[..].into()), Atom(b"test"[..].into())])
                            },
                        },
                        MailboxRef {
                            name: Some(Phrase(vec![
                                PhraseToken::Word(Word::Atom(Atom(b"John"[..].into()))),
                            ])),
                            addrspec: AddrSpec {
                                local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(Atom(b"jdoe"[..].into())))]),
                                domain: Domain::Atoms(vec![Atom(b"one"[..].into()), Atom(b"test"[..].into())])
                            },
                        },
                    ])),
                }),
                AddressRef::Single(MailboxRef {
                    name: Some(Phrase(vec![
                        PhraseToken::Word(Word::Atom(Atom(b"Mary"[..].into()))),
                        PhraseToken::Word(Word::Atom(Atom(b"Smith"[..].into()))),
                    ])),
                    addrspec: AddrSpec {
                        local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(Atom(b"mary"[..].into())))]),
                        domain: Domain::Atoms(vec![Atom(b"x"[..].into()), Atom(b"test"[..].into())])
                    },
                }),
            ],
        );
    }

    #[test]
    fn test_address_list_obs() {
        address_list_reprinted(
            br#"  ,,A Group:Ed Jones <c@a.test>,,,,joe@where.test,John <jdoe@one.test>;, Mary Smith <mary@x.test>,,"#,
            br#"A Group:Ed Jones <c@a.test>, joe@where.test, John <jdoe@one.test>;, Mary Smith <mary@x.test>"#,
        )
    }

    use crate::text::encoding::{EncodedWord, EncodedWordToken, QuotedChunk, QuotedWord};
    use crate::text::quoted::QuotedString;

    #[test]
    fn test_strange_groups() {
        address_list_parsed_printed(
            br#""Colleagues": "James Smythe" <james@vandelay.com>;, Friends:
  jane@example.com, =?UTF-8?Q?John_Sm=C3=AEth?= <john@example.com>;"#,
            br#""Colleagues":"James Smythe" <james@vandelay.com>;, Friends:jane@example.com, =?UTF-8?Q?John_Sm=C3=AEth?= <john@example.com>;"#,
            vec![
                AddressRef::Many(GroupRef {
                    name: Phrase(vec![
                        PhraseToken::Word(Word::Quoted(QuotedString(vec![b"Colleagues"[..].into()]))),
                    ]),
                    participants: Some(MailboxList(vec![MailboxRef {
                        name: Some(Phrase(vec![
                            PhraseToken::Word(Word::Quoted(QuotedString(vec![
                                b"James"[..].into(),
                                b" "[..].into(),
                                b"Smythe"[..].into(),
                            ])))])),
                        addrspec: AddrSpec {
                            local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(
                                Atom(b"james"[..].into())
                            ))]),
                            domain: Domain::Atoms(vec![Atom(b"vandelay"[..].into()), Atom(b"com"[..].into())]),
                        }
                    },])),
                }),
                AddressRef::Many(GroupRef {
                    name: Phrase(vec![PhraseToken::Word(Word::Atom(Atom(b"Friends"[..].into())))]),
                    participants: Some(MailboxList(vec![
                        MailboxRef {
                            name: None,
                            addrspec: AddrSpec {
                                local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(
                                    Atom(b"jane"[..].into())
                                ))]),
                                domain: Domain::Atoms(vec![Atom(b"example"[..].into()), Atom(b"com"[..].into())]),
                            }
                        },
                        MailboxRef {
                            name: Some(Phrase(vec![PhraseToken::Encoded(EncodedWord(vec![
                                EncodedWordToken::Quoted(
                                    QuotedWord {
                                        enc: EmailCharset::UTF_8,
                                        chunks: vec![
                                            QuotedChunk::Safe(b"John"[..].into()),
                                            QuotedChunk::Space,
                                            QuotedChunk::Safe(b"Sm"[..].into()),
                                            QuotedChunk::Encoded(vec![0xc3, 0xae]),
                                            QuotedChunk::Safe(b"th"[..].into()),
                                        ]
                                    }
                                )
                            ]))])),
                            addrspec: AddrSpec {
                                local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(
                                    Atom(b"john"[..].into())
                                ))]),
                                domain: Domain::Atoms(vec![Atom(b"example"[..].into()), Atom(b"com"[..].into())]),
                            }
                        },
                    ]))
                }),
            ],
        );

        address_list_parsed_printed(
            b"group:;",
            b"group:;",
            vec![AddressRef::Many(GroupRef {
                name: Phrase(vec![PhraseToken::Word(Word::Atom(Atom(b"group".into())))]),
                participants: None,
            })],
        );

        address_list_parsed_printed(
            b"group: \r\n ;",
            b"group:;",
            vec![AddressRef::Many(GroupRef {
                name: Phrase(vec![PhraseToken::Word(Word::Atom(Atom(b"group".into())))]),
                participants: None,
            })],
        );
    }

    #[test]
    fn test_obs_groups() {
        address_list_parsed_printed(
            b"group: ,,  \r\n  ,,,, ;",
            b"group:;",
            vec![AddressRef::Many(GroupRef {
                name: Phrase(vec![PhraseToken::Word(Word::Atom(Atom(b"group".into())))]),
                participants: None,
            })],
        );

        address_list_parsed_printed(
            b"group:,;",
            b"group:;",
            vec![AddressRef::Many(GroupRef {
                name: Phrase(vec![PhraseToken::Word(Word::Atom(Atom(b"group".into())))]),
                participants: None,
            })],
        )
    }
}
