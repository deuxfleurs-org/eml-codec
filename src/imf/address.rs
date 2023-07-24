use nom::{
    branch::alt,
    bytes::complete::tag,
    combinator::{into, map, opt},
    multi::separated_list1,
    sequence::tuple,
    IResult,
};

//use crate::error::IMFError;
use crate::imf::mailbox::{mailbox, MailboxRef};
use crate::text::misc_token::{phrase, Phrase};
use crate::text::whitespace::cfws;

#[derive(Debug, PartialEq)]
pub struct GroupRef<'a> {
    pub name: Phrase<'a>,
    pub participants: Vec<MailboxRef<'a>>,
}

#[derive(Debug, PartialEq)]
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
pub type AddressList<'a> = Vec<AddressRef<'a>>;

/// Address (section 3.4 of RFC5322)
///
/// ```abnf
///    address         =   mailbox / group
/// ```
pub fn address(input: &[u8]) -> IResult<&[u8], AddressRef> {
    alt((into(mailbox), into(group)))(input)
}

/// Group
///
/// ```abnf
///    group           =   display-name ":" [group-list] ";" [CFWS]
///    display-name    =   phrase
/// ```
pub fn group(input: &[u8]) -> IResult<&[u8], GroupRef> {
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
pub fn group_list(input: &[u8]) -> IResult<&[u8], Vec<MailboxRef>> {
    alt((mailbox_list, mailbox_cfws))(input)
}

fn mailbox_cfws(input: &[u8]) -> IResult<&[u8], Vec<MailboxRef>> {
    let (input, _) = cfws(input)?;
    Ok((input, vec![]))
}

/// Mailbox list
///
/// ```abnf
///    mailbox-list    =   (mailbox *("," mailbox)) / obs-mbox-list
/// ```
pub fn mailbox_list(input: &[u8]) -> IResult<&[u8], Vec<MailboxRef>> {
    separated_list1(tag(","), mailbox)(input)
}

/// Address list
///
/// ```abnf
///   address-list    =   (address *("," address)) / obs-addr-list
/// ```
pub fn address_list(input: &[u8]) -> IResult<&[u8], Vec<AddressRef>> {
    separated_list1(tag(","), address)(input)
}

pub fn address_list_cfws(input: &[u8]) -> IResult<&[u8], Vec<AddressRef>> {
    let (input, _) = cfws(input)?;
    Ok((input, vec![]))
}

pub fn nullable_address_list(input: &[u8]) -> IResult<&[u8], Vec<AddressRef>> {
    map(opt(alt((address_list, address_list_cfws))), |v| {
        v.unwrap_or(vec![])
    })(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::imf::mailbox::{AddrSpec, Domain, LocalPart, LocalPartToken};
    use crate::text::misc_token::{Phrase, Word};

    #[test]
    fn test_mailbox_list() {
        match mailbox_list(
            r#"Pete(A nice \) chap) <pete(his account)@silly.test(his host)>"#.as_bytes(),
        ) {
            Ok((rest, _)) => assert_eq!(&b""[..], rest),
            _ => panic!(),
        };

        match mailbox_list(
            r#"Mary Smith <mary@x.test>, jdoe@example.org, Who? <one@y.test>, <boss@nil.test>, "Giant; \"Big\" Box" <sysservices@example.net>"#.as_bytes(),
        ) {
            Ok((rest, _)) => assert_eq!(&b""[..], rest),
            _ => panic!(),
        };
    }

    #[test]
    fn test_address_list() {
        assert_eq!(
            address_list(
                r#"A Group:Ed Jones <c@a.test>,joe@where.test,John <jdoe@one.test>;, Mary Smith <mary@x.test>"#.as_bytes()
            ),
            Ok((
                &b""[..],
                vec![
                    AddressRef::Many(GroupRef {
                        name: Phrase(vec![Word::Atom(&b"A"[..]), Word::Atom(&b"Group"[..])]),
                        participants: vec![
                            MailboxRef {
                                name: Some(Phrase(vec![Word::Atom(&b"Ed"[..]), Word::Atom(&b"Jones"[..])])),
                                addrspec: AddrSpec {
                                    local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(&b"c"[..]))]),
                                    domain: Domain::Atoms(vec![&b"a"[..], &b"test"[..]]),
                                },
                            },
                            MailboxRef {
                                name: None,
                                addrspec: AddrSpec {
                                    local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(&b"joe"[..]))]),
                                    domain: Domain::Atoms(vec![&b"where"[..], &b"test"[..]])
                                },
                            },
                            MailboxRef {
                                name: Some(Phrase(vec![Word::Atom(&b"John"[..])])),
                                addrspec: AddrSpec {
                                    local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(&b"jdoe"[..]))]),
                                    domain: Domain::Atoms(vec![&b"one"[..], &b"test"[..]])
                                },
                            },
                        ],
                    }),
                    AddressRef::Single(MailboxRef {
                        name: Some(Phrase(vec![Word::Atom(&b"Mary"[..]), Word::Atom(&b"Smith"[..])])),
                        addrspec: AddrSpec {
                            local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(&b"mary"[..]))]),
                            domain: Domain::Atoms(vec![&b"x"[..], &b"test"[..]])
                        },
                    }),
                ]
            ))
        );
    }

    use crate::text::encoding::{EncodedWord, QuotedChunk, QuotedWord};
    use crate::text::quoted::QuotedString;

    #[test]
    fn test_strange_groups() {
        assert_eq!(
            address_list(
                br#""Colleagues": "James Smythe" <james@vandelay.com>;, Friends:
  jane@example.com, =?UTF-8?Q?John_Sm=C3=AEth?= <john@example.com>;"#
            ),
            Ok((
                &b""[..],
                vec![
                    AddressRef::Many(GroupRef {
                        name: Phrase(vec![Word::Quoted(QuotedString(vec![&b"Colleagues"[..]]))]),
                        participants: vec![MailboxRef {
                            name: Some(Phrase(vec![Word::Quoted(QuotedString(vec![
                                &b"James"[..],
                                &b" "[..],
                                &b"Smythe"[..]
                            ]))])),
                            addrspec: AddrSpec {
                                local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(
                                    &b"james"[..]
                                ))]),
                                domain: Domain::Atoms(vec![&b"vandelay"[..], &b"com"[..]]),
                            }
                        },],
                    }),
                    AddressRef::Many(GroupRef {
                        name: Phrase(vec![Word::Atom(&b"Friends"[..])]),
                        participants: vec![
                            MailboxRef {
                                name: None,
                                addrspec: AddrSpec {
                                    local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(
                                        &b"jane"[..]
                                    ))]),
                                    domain: Domain::Atoms(vec![&b"example"[..], &b"com"[..]]),
                                }
                            },
                            MailboxRef {
                                name: Some(Phrase(vec![Word::Encoded(EncodedWord::Quoted(
                                    QuotedWord {
                                        enc: encoding_rs::UTF_8,
                                        chunks: vec![
                                            QuotedChunk::Safe(&b"John"[..]),
                                            QuotedChunk::Space,
                                            QuotedChunk::Safe(&b"Sm"[..]),
                                            QuotedChunk::Encoded(vec![0xc3, 0xae]),
                                            QuotedChunk::Safe(&b"th"[..]),
                                        ]
                                    }
                                ))])),
                                addrspec: AddrSpec {
                                    local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(
                                        &b"john"[..]
                                    ))]),
                                    domain: Domain::Atoms(vec![&b"example"[..], &b"com"[..]]),
                                }
                            },
                        ]
                    }),
                ]
            ))
        );
    }
}
