use std::collections::HashMap;
use chrono::{DateTime,FixedOffset,ParseError};

#[derive(Debug, PartialEq, Default)]
pub enum HeaderDate {
    Parsed(DateTime<FixedOffset>),
    Unknown(String, ParseError),
    #[default]
    None,
}

#[derive(Debug, PartialEq)]
pub struct AddrSpec {
    pub local_part: String,
    pub domain: String,
}
impl AddrSpec {
    pub fn fully_qualified(&self) -> String {
        format!("{}@{}", self.local_part, self.domain)
    }
}

#[derive(Debug, PartialEq)]
pub struct MailboxRef {
    // The actual "email address" like hello@example.com
    pub addrspec: AddrSpec,
    pub name: Option<String>,
}
impl From<AddrSpec> for MailboxRef {
    fn from(addr: AddrSpec) -> Self {
        MailboxRef {
            name: None,
            addrspec: addr,
        }
    }
}

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

#[derive(Debug, PartialEq)]
pub struct MessageId<'a> {
    pub left: &'a str,
    pub right: &'a str,
}

#[derive(Debug, PartialEq)]
pub struct Trace {
    // 3.6.7 Traces
    pub received: Vec<String>,
    pub return_path: Option<String>,

    // 3.6.6.  Resent Fields
    pub resent_date: HeaderDate,
    pub resent_from: Vec<MailboxRef>,
    pub resent_sender: Option<MailboxRef>,
    pub resent_to: Vec<AddressRef>,
    pub resent_cc: Vec<AddressRef>,
    pub resent_bcc: Vec<AddressRef>,
    pub resent_msg_id: Option<MessageId<'a>>,

    // 3.6.8.  Optional Fields
    pub optional: HashMap<&'a str, String>,
    //pub failed: HashMap<&'a str, String>,
    //pub garbage: &'a str,
}

/// Permissive Header Section
///
/// This is a structure intended for parsing/decoding,
/// hence it's support cases where the email is considered
/// as invalid according to RFC5322 but for which we can
/// still extract some data.
#[derive(Debug, Default)]
pub struct CommonFields<'a> {
    // 3.6.1.  The Origination Date Field
    pub date: HeaderDate,

    // 3.6.2.  Originator Fields
    pub from: Vec<MailboxRef>,
    pub sender: Option<MailboxRef>,
    pub reply_to: Vec<AddressRef>,

    // 3.6.3.  Destination Address Fields
    pub to: Vec<AddressRef>,
    pub cc: Vec<AddressRef>,
    pub bcc: Vec<AddressRef>,

    // 3.6.4.  Identification Fields
    pub msg_id: Option<MessageId<'a>>,
    pub in_reply_to: Vec<MessageId<'a>>,
    pub references: Vec<MessageId<'a>>,
    
    // 3.6.5.  Informational Fields
    pub subject: Option<String>,
    pub comments: Vec<String>,
    pub keywords: Vec<String>,

    // 3.6.8.  Optional Fields
    pub optional: HashMap<&'a str, String>,
    //pub failed: HashMap<&'a str, String>,
    //pub garbage: &'a str,
}

pub struct HeaderSection<'a> {
    // 3.6.7 Traces
    pub traces: Vec<Trace>,

    // 3.6.x
    pub common: CommonFields,
}

enum InvalidEmailErr {
    NoUsableDate,
}

impl<'a> PermissiveHeaderSection<'a> {
    /// Check validity of the email
    ///
    /// Especially check that there is no missing fields,
    /// or no unique fields declared multiple times.
    ///
    /// See: https://www.rfc-editor.org/rfc/rfc5322#section-3.6
    //@FIXME could be changed to a to_StrictHeaderSection call. All fixed errors would be returned in
    // a vec of errors.
    fn is_valid(&self) -> Result<(), InvalidEmailErr> {
        match self.date {
            HeaderDate::Parsed(_) => (),
            _ => return Err(InvalidEmailErr::NoUsableDate),
        };

        Ok(())
    }
}
