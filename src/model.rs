use std::collections::HashMap;
use chrono::{DateTime,FixedOffset,ParseError};

#[derive(Debug, Default)]
pub enum HeaderDate {
    Parsed(DateTime<FixedOffset>),
    Unknown(String, ParseError),
    #[default]
    None,
}

#[derive(Debug)]
pub struct MailboxRef<'a> {
    // The actual "email address" like hello@example.com
    pub addrspec: &'a str,
    pub name: Option<&'a str>,
}

#[derive(Debug)]
pub struct GroupRef<'a> {
    pub name: &'a str,
    pub mbx: Vec<MailboxRef<'a>>,
}

#[derive(Debug)]
pub enum AddressRef<'a> {
    Single(MailboxRef<'a>),
    Many(GroupRef<'a>),
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
    pub from: Vec<MailboxRef<'a>>,
    pub sender: Option<MailboxRef<'a>>,
    pub reply_to: Vec<AddressRef<'a>>,
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
