use std::collections::HashMap;
use chrono::{DateTime,FixedOffset,ParseError};

#[derive(Debug, Default)]
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

#[derive(Debug)]
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

#[derive(Debug)]
pub struct GroupRef {
    pub name: String,
    pub mbx: Vec<MailboxRef>,
}

#[derive(Debug)]
pub enum AddressRef {
    Single(MailboxRef),
    Many(GroupRef),
}

/// Permissive Header Section
///
/// This is a structure intended for parsing/decoding,
/// hence it's support cases where the email is considered
/// as invalid according to RFC5322 but for which we can
/// still extract some data.
#[derive(Debug, Default)]
pub struct PermissiveHeaderSection<'a> {
    pub subject: Option<String>,
    pub from: Vec<MailboxRef>,
    pub sender: Option<MailboxRef>,
    pub reply_to: Vec<AddressRef>,
    pub date: HeaderDate,
    pub optional: HashMap<&'a str, String>,
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
