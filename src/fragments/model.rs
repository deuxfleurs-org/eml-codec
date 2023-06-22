use std::collections::HashMap;
use chrono::{DateTime,FixedOffset};

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
pub type MailboxList = Vec<MailboxRef>;

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

#[derive(Debug, PartialEq)]
pub struct MessageId<'a> {
    pub left: &'a str,
    pub right: &'a str,
}
pub type MessageIdList<'a> = Vec<MessageId<'a>>;

#[derive(Debug, PartialEq)]
pub enum FieldBody<'a, T> {
    Correct(T),
    Failed(&'a str),
}

#[derive(Debug, PartialEq)]
pub enum Field<'a> {
    // 3.6.1.  The Origination Date Field
    Date(FieldBody<'a, Option<DateTime<FixedOffset>>>),

    // 3.6.2.  Originator Fields
    From(FieldBody<'a, Vec<MailboxRef>>),
    Sender(FieldBody<'a, MailboxRef>),
    ReplyTo(FieldBody<'a, Vec<AddressRef>>),

    // 3.6.3.  Destination Address Fields
    To(FieldBody<'a, Vec<AddressRef>>),
    Cc(FieldBody<'a, Vec<AddressRef>>),
    Bcc(FieldBody<'a, Vec<AddressRef>>),

    // 3.6.4.  Identification Fields
    MessageID(FieldBody<'a, MessageId<'a>>),
    InReplyTo(FieldBody<'a, Vec<MessageId<'a>>>),
    References(FieldBody<'a, Vec<MessageId<'a>>>),

    // 3.6.5.  Informational Fields
    Subject(FieldBody<'a, String>),
    Comments(FieldBody<'a, String>),
    Keywords(FieldBody<'a, Vec<String>>),

    // 3.6.6   Resent Fields (not implemented)
    // 3.6.7   Trace Fields
    Received(FieldBody<'a, &'a str>),
    ReturnPath(FieldBody<'a, Option<MailboxRef>>),

    // 3.6.8.  Optional Fields
    Optional(&'a str, String),

    // None
    Rescue(&'a str),
}

/// Permissive Header Section
///
/// This is a structure intended for parsing/decoding,
/// hence it's support cases where the email is considered
/// as invalid according to RFC5322 but for which we can
/// still extract some data.
#[derive(Debug, PartialEq, Default)]
pub struct HeaderSection<'a> {
    // 3.6.1.  The Origination Date Field
    pub date: Option<DateTime<FixedOffset>>,

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

    // 3.6.6 Not implemented
    // 3.6.7 Trace Fields
    pub return_path: Vec<MailboxRef>,
    pub received: Vec<&'a str>,

    // 3.6.8.  Optional Fields
    pub optional: HashMap<&'a str, String>,

    // Recovery
    pub bad_fields: Vec<Field<'a>>,
    pub unparsed: Vec<&'a str>,
}
