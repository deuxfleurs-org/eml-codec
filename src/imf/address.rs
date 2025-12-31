use bounded_static::ToStatic;
use nom::{
    branch::alt,
    bytes::complete::tag,
    combinator::{into, map, opt},
    multi::separated_list1,
    sequence::tuple,
    IResult,
};

//use crate::error::IMFError;
use crate::display_bytes::{print_seq, Print, Formatter};
use crate::imf::mailbox::{mailbox, MailboxRef};
use crate::text::misc_token::{phrase, Phrase};
use crate::text::whitespace::cfws;

#[derive(Clone, Debug, PartialEq, ToStatic)]
pub struct GroupRef<'a> {
    pub name: Phrase<'a>,
    pub participants: Vec<MailboxRef<'a>>,
}
impl<'a> Print for GroupRef<'a> {
    fn print(&self, fmt: &mut impl Formatter) -> std::io::Result<()> {
        self.name.print(fmt)?;
        fmt.write_bytes(b":")?;
        self.participants.print(fmt)?;
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
    fn print(&self, fmt: &mut impl Formatter) -> std::io::Result<()> {
        match self {
            AddressRef::Single(mbox) => mbox.print(fmt),
            AddressRef::Many(group) => group.print(fmt),
        }
    }
}

pub type AddressList<'a> = Vec<AddressRef<'a>>;

impl<'a> Print for AddressList<'a> {
    fn print(&self, fmt: &mut impl Formatter) -> std::io::Result<()> {
        print_seq(fmt, self, |fmt| {
            fmt.write_bytes(b",")?;
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
            participants: grp_list.unwrap_or(vec![]),
        },
    ))
}

/// Group list
///
/// ```abnf
///    group-list      =   mailbox-list / CFWS / obs-group-list
/// ```
// TODO: obs-group-list
pub fn group_list(input: &[u8]) -> IResult<&[u8], Vec<MailboxRef<'_>>> {
    alt((mailbox_list, mailbox_cfws))(input)
}

fn mailbox_cfws(input: &[u8]) -> IResult<&[u8], Vec<MailboxRef<'_>>> {
    let (input, _) = cfws(input)?;
    Ok((input, vec![]))
}

/// Mailbox list
///
/// ```abnf
///    mailbox-list    =   (mailbox *("," mailbox)) / obs-mbox-list
/// ```
// TODO: obs-mbox-list
// TODO: move to mailbox.rs?
pub fn mailbox_list(input: &[u8]) -> IResult<&[u8], Vec<MailboxRef<'_>>> {
    separated_list1(tag(","), mailbox)(input)
}

/// Address list
///
/// ```abnf
///   address-list    =   (address *("," address)) / obs-addr-list
/// ```
// TODO: obs-addr-list
pub fn address_list(input: &[u8]) -> IResult<&[u8], Vec<AddressRef<'_>>> {
    separated_list1(tag(","), address)(input)
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
    use crate::imf::mailbox::{AddrSpec, Domain, LocalPart, LocalPartToken};
    use crate::text::misc_token::{Phrase, PhraseToken, Word};

    fn mailbox_list_reprint(mboxlist: &[u8], printed: &[u8]) {
        let (input, parsed) = mailbox_list(mboxlist).unwrap();
        assert!(input.is_empty());
        let mut v = Vec::new();
        parsed.print(&mut v).unwrap();
        assert_eq!(String::from_utf8_lossy(&v), String::from_utf8_lossy(printed));
    }

    fn address_list_parsed_printed(addrlist: &[u8], printed: &[u8], parsed: AddressList<'_>) {
        assert_eq!(address_list(addrlist).unwrap(), (&b""[..], parsed.clone()));
        let mut v = Vec::new();
        parsed.print(&mut v).unwrap();
        assert_eq!(String::from_utf8_lossy(&v), String::from_utf8_lossy(printed));
    }

    #[test]
    fn test_mailbox_list() {
        mailbox_list_reprint(
            r#"Pete(A nice \) chap) <pete(his account)@silly.test(his host)>"#.as_bytes(),
            b"Pete <pete@silly.test>",
        );

        mailbox_list_reprint(
            r#"Mary Smith <mary@x.test>, jdoe@example.org, Who? <one@y.test>, <boss@nil.test>, "Giant; \"Big\" Box" <sysservices@example.net>"#.as_bytes(),
            r#"Mary Smith <mary@x.test>, jdoe@example.org, Who? <one@y.test>, boss@nil.test, "Giant; \"Big\" Box" <sysservices@example.net>"#.as_bytes(),
        );
    }

    #[test]
    fn test_address_list() {
        address_list_parsed_printed(
            r#"A Group:Ed Jones <c@a.test>,joe@where.test,John <jdoe@one.test>;, Mary Smith <mary@x.test>"#.as_bytes(),
            r#"A Group:Ed Jones <c@a.test>, joe@where.test, John <jdoe@one.test>;, Mary Smith <mary@x.test>"#.as_bytes(),
            vec![
                AddressRef::Many(GroupRef {
                    name: Phrase(vec![
                        PhraseToken::Word(Word::Atom(b"A"[..].into())),
                        PhraseToken::Word(Word::Atom(b"Group"[..].into())),
                    ]),
                    participants: vec![
                        MailboxRef {
                            name: Some(Phrase(vec![
                                PhraseToken::Word(Word::Atom(b"Ed"[..].into())),
                                PhraseToken::Word(Word::Atom(b"Jones"[..].into())),
                            ])),
                            addrspec: AddrSpec {
                                local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(b"c"[..].into()))]),
                                domain: Domain::Atoms(vec![b"a"[..].into(), b"test"[..].into()]),
                            },
                        },
                        MailboxRef {
                            name: None,
                            addrspec: AddrSpec {
                                local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(b"joe"[..].into()))]),
                                domain: Domain::Atoms(vec![b"where"[..].into(), b"test"[..].into()])
                            },
                        },
                        MailboxRef {
                            name: Some(Phrase(vec![
                                PhraseToken::Word(Word::Atom(b"John"[..].into())),
                            ])),
                            addrspec: AddrSpec {
                                local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(b"jdoe"[..].into()))]),
                                domain: Domain::Atoms(vec![b"one"[..].into(), b"test"[..].into()])
                            },
                        },
                    ],
                }),
                AddressRef::Single(MailboxRef {
                    name: Some(Phrase(vec![
                        PhraseToken::Word(Word::Atom(b"Mary"[..].into())),
                        PhraseToken::Word(Word::Atom(b"Smith"[..].into())),
                    ])),
                    addrspec: AddrSpec {
                        local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(b"mary"[..].into()))]),
                        domain: Domain::Atoms(vec![b"x"[..].into(), b"test"[..].into()])
                    },
                }),
            ],
        );
    }

    use crate::text::encoding::{EncodedWord, QuotedChunk, QuotedWord};
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
                    participants: vec![MailboxRef {
                        name: Some(Phrase(vec![
                            PhraseToken::Word(Word::Quoted(QuotedString(vec![
                                b"James"[..].into(),
                                b" "[..].into(),
                                b"Smythe"[..].into(),
                            ])))])),
                        addrspec: AddrSpec {
                            local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(
                                b"james"[..].into()
                            ))]),
                            domain: Domain::Atoms(vec![b"vandelay"[..].into(), b"com"[..].into()]),
                        }
                    },],
                }),
                AddressRef::Many(GroupRef {
                    name: Phrase(vec![PhraseToken::Word(Word::Atom(b"Friends"[..].into()))]),
                    participants: vec![
                        MailboxRef {
                            name: None,
                            addrspec: AddrSpec {
                                local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(
                                    b"jane"[..].into()
                                ))]),
                                domain: Domain::Atoms(vec![b"example"[..].into(), b"com"[..].into()]),
                            }
                        },
                        MailboxRef {
                            name: Some(Phrase(vec![PhraseToken::Encoded(EncodedWord::Quoted(
                                QuotedWord {
                                    enc: encoding_rs::UTF_8,
                                    chunks: vec![
                                        QuotedChunk::Safe(b"John"[..].into()),
                                        QuotedChunk::Space,
                                        QuotedChunk::Safe(b"Sm"[..].into()),
                                        QuotedChunk::Encoded(vec![0xc3, 0xae]),
                                        QuotedChunk::Safe(b"th"[..].into()),
                                    ]
                                }
                            ))])),
                            addrspec: AddrSpec {
                                local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(
                                    b"john"[..].into()
                                ))]),
                                domain: Domain::Atoms(vec![b"example"[..].into(), b"com"[..].into()]),
                            }
                        },
                    ]
                }),
            ],
        );
    }
}
