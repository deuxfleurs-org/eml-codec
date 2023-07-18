use nom::{
    branch::alt,
    bytes::complete::tag,
    combinator::{into, opt},
    multi::separated_list1,
    sequence::tuple,
    IResult,
};

use crate::error::IMFError;
use crate::fragments::lazy;
use crate::fragments::mailbox::mailbox;
use crate::fragments::misc_token::phrase;
//use crate::fragments::model::{AddressList, AddressRef, GroupRef, MailboxList, MailboxRef};
use crate::fragments::whitespace::cfws;

#[derive(Debug, PartialEq)]
pub struct GroupRef {
    pub name: String,
    pub participants: Vec<MailboxRef>,
}

#[derive(Debug, PartialEq)]
pub enum AddressRef {
    Single(MailboxRef),
    Many(GroupRef),
}
impl From<MailboxRef> for AddressRef {
    fn from(mx: MailboxRef) -> Self {
        AddressRef::Single(mx)
    }
}
impl From<GroupRef> for AddressRef {
    fn from(grp: GroupRef) -> Self {
        AddressRef::Many(grp)
    }
}
pub type AddressList = Vec<AddressRef>;

impl<'a> TryFrom<&'a lazy::Mailbox<'a>> for MailboxRef {
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
}

/// Address (section 3.4 of RFC5322)
///
/// ```abnf
///    address         =   mailbox / group
/// ```
pub fn address(input: &str) -> IResult<&str, AddressRef> {
    alt((into(mailbox), into(group)))(input)
}

/// Group
///
/// ```abnf
///    group           =   display-name ":" [group-list] ";" [CFWS]
///    display-name    =   phrase
/// ```
pub fn group(input: &str) -> IResult<&str, GroupRef> {
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
pub fn group_list(input: &str) -> IResult<&str, Vec<MailboxRef>> {
    alt((mailbox_list, mailbox_cfws))(input)
}

fn mailbox_cfws(input: &str) -> IResult<&str, Vec<MailboxRef>> {
    let (input, _) = cfws(input)?;
    Ok((input, vec![]))
}

/// Mailbox list
///
/// ```abnf
///    mailbox-list    =   (mailbox *("," mailbox)) / obs-mbox-list
/// ```
pub fn mailbox_list(input: &str) -> IResult<&str, Vec<MailboxRef>> {
    separated_list1(tag(","), mailbox)(input)
}

/// Address list
///
/// ```abnf
///   address-list    =   (address *("," address)) / obs-addr-list
/// ```
pub fn address_list(input: &str) -> IResult<&str, Vec<AddressRef>> {
    separated_list1(tag(","), address)(input)
}

pub fn address_list_cfws(input: &str) -> IResult<&str, Vec<AddressRef>> {
    let (input, _) = cfws(input)?;
    Ok((input, vec![]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fragments::model::AddrSpec;

    #[test]
    fn test_mailbox_list() {
        match mailbox_list(r#"Pete(A nice \) chap) <pete(his account)@silly.test(his host)>"#) {
            Ok(("", _)) => (),
            _ => panic!(),
        };

        match mailbox_list(
            r#"Mary Smith <mary@x.test>, jdoe@example.org, Who? <one@y.test>, <boss@nil.test>, "Giant; \"Big\" Box" <sysservices@example.net>"#,
        ) {
            Ok(("", _)) => (),
            _ => panic!(),
        };
    }

    #[test]
    fn test_address_list() {
        assert_eq!(
            address_list(
                r#"A Group:Ed Jones <c@a.test>,joe@where.test,John <jdoe@one.test>;, Mary Smith <mary@x.test>"#
            ),
            Ok((
                "",
                vec![
                    AddressRef::Many(GroupRef {
                        name: "A Group".to_string(),
                        participants: vec![
                            MailboxRef {
                                name: Some("Ed Jones".into()),
                                addrspec: AddrSpec {
                                    local_part: "c".into(),
                                    domain: "a.test".into()
                                },
                            },
                            MailboxRef {
                                name: None,
                                addrspec: AddrSpec {
                                    local_part: "joe".into(),
                                    domain: "where.test".into()
                                },
                            },
                            MailboxRef {
                                name: Some("John".into()),
                                addrspec: AddrSpec {
                                    local_part: "jdoe".into(),
                                    domain: "one.test".into()
                                },
                            },
                        ],
                    }),
                    AddressRef::Single(MailboxRef {
                        name: Some("Mary Smith".into()),
                        addrspec: AddrSpec {
                            local_part: "mary".into(),
                            domain: "x.test".into()
                        },
                    }),
                ]
            ))
        );
    }
}
