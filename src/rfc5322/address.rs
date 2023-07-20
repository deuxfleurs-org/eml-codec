use nom::{
    branch::alt,
    bytes::complete::tag,
    combinator::{into, map, opt},
    multi::separated_list1,
    sequence::tuple,
    IResult,
};

//use crate::error::IMFError;
use crate::rfc5322::mailbox::{mailbox, MailboxRef};
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

/*impl<'a> TryFrom<&'a lazy::Mailbox<'a>> for MailboxRef {
    type Error = IMFError<'a>;

    fn try_from(mx: &'a lazy::Mailbox<'a>) -> Result<Self, Self::Error> {
        mailbox(mx.0)
            .map(|(_, m)| m)
            .map_err(|e| IMFError::Mailbox(e))
    }
}

impl<'a> TryFrom<&'a lazy::MailboxList<'a>> for MailboxList {
    type Error = IMFError<'a>;

    fn try_from(ml: &'a lazy::MailboxList<'a>) -> Result<Self, Self::Error> {
        mailbox_list(ml.0)
            .map(|(_, m)| m)
            .map_err(|e| IMFError::MailboxList(e))
    }
}

impl<'a> TryFrom<&'a lazy::AddressList<'a>> for AddressList {
    type Error = IMFError<'a>;

    fn try_from(al: &'a lazy::AddressList<'a>) -> Result<Self, Self::Error> {
        address_list(al.0)
            .map(|(_, a)| a)
            .map_err(|e| IMFError::AddressList(e))
    }
}

impl<'a> TryFrom<&'a lazy::NullableAddressList<'a>> for AddressList {
    type Error = IMFError<'a>;

    fn try_from(nal: &'a lazy::NullableAddressList<'a>) -> Result<Self, Self::Error> {
        opt(alt((address_list, address_list_cfws)))(nal.0)
            .map(|(_, a)| a.unwrap_or(vec![]))
            .map_err(|e| IMFError::NullableAddressList(e))
    }
}*/

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
    map(
        opt(alt((address_list, address_list_cfws))), 
        |v| v.unwrap_or(vec![]),
    )(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::misc_token::{Phrase, Word};
    use crate::rfc5322::mailbox::{AddrSpec, Domain, LocalPart, LocalPartToken};

    #[test]
    fn test_mailbox_list() {
        match mailbox_list(r#"Pete(A nice \) chap) <pete(his account)@silly.test(his host)>"#.as_bytes()) {
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
}
