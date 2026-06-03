/// Parse and represent IMF (Internet Message Format) headers (RFC822, RFC5322)
#[cfg(feature = "arbitrary")]
use arbitrary::Arbitrary;
pub mod address;
pub mod datetime;
pub mod field;
pub mod identification;
pub mod mailbox;
pub mod mime;
pub mod trace;

use bounded_static::ToStatic;
use std::collections::HashSet;

#[cfg(feature = "arbitrary")]
use crate::fuzz_eq::FuzzEq;
use crate::i18n::ContainsUtf8;
use crate::imf::address::AddressRef;
use crate::imf::datetime::DateTime;
use crate::imf::field::{Entry, Field};
use crate::imf::identification::MessageID;
use crate::imf::mailbox::{MailboxList, MailboxRef};
use crate::imf::mime::Version;
use crate::imf::trace::ReturnPath;
use crate::text::misc_token::{PhraseList, Unstructured};

#[derive(Clone, ContainsUtf8, Debug, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary, FuzzEq))]
pub struct Imf<'a> {
    // 3.6.1.  The Origination Date Field
    pub date: DateTimeOpt,

    // 3.6.2.  Originator Fields
    pub from: From<'a>, // combines 'from' and 'sender'
    pub reply_to: Vec<AddressRef<'a>>,

    // 3.6.3.  Destination Address Fields
    pub to: Vec<AddressRef<'a>>,
    pub cc: Vec<AddressRef<'a>>,
    pub bcc: Option<Vec<AddressRef<'a>>>,

    // 3.6.4.  Identification Fields
    pub msg_id: Option<MessageID<'a>>,
    pub in_reply_to: Vec<MessageID<'a>>,
    pub references: Vec<MessageID<'a>>,

    // 3.6.5.  Informational Fields
    pub subject: Option<Unstructured<'a>>,
    pub comments: Vec<Unstructured<'a>>,
    pub keywords: Vec<PhraseList<'a>>,

    // 3.6.6 Not implemented

    // 3.6.7 Trace Fields
    pub trace: Vec<TraceField<'a>>,

    // MIME
    pub mime_version: Option<Version>,
}

#[derive(Clone, ContainsUtf8, Debug, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary, FuzzEq))]
pub enum DateTimeOpt {
    Some(DateTime),
    // Following RFC5322, it is invalid for the Date header to be missing
    // However, IMAP RFCs allow the Date header to be missing (e.g. for draft
    // emails) (in particular, RFC9051 "IMAP4rev2" makes it clear in §7.5.2
    // ENVELOPE).
    InvalidMissing,
}

#[derive(Clone, ContainsUtf8, Debug, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub enum From<'a> {
    Single {
        from: MailboxRef<'a>,
        sender: Option<MailboxRef<'a>>,
    },
    Multiple {
        from: MailboxList<'a>, // must contain at least two elements
        sender: MailboxRef<'a>,
    },
    // Following RFC5322, it is invalid for the From header to be missing.
    // However, IMAP RFCs allow it to be missing (e.g. for draft emails).
    // This also represents the case where both From and Sender are missing,
    // as `InvalidMissingFrom { sender: None }`.
    InvalidMissingFrom {
        sender: Option<MailboxRef<'a>>,
    },
    // Following RFC5322, it is invalid for the Sender header to be missing
    // if there are more than one From mailbox.
    // However, IMAP RFCs allow it to be missing (e.g. for draft emails).
    InvalidMissingSender {
        from: MailboxList<'a>, // must contain at least two elements
    },
}

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for From<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        match u.int_in_range(0..=3)? {
            0 => Ok(From::Single {
                from: u.arbitrary()?,
                sender: u.arbitrary()?,
            }),
            1 => {
                let mut from: MailboxList = u.arbitrary()?;
                from.0.push(u.arbitrary()?);
                Ok(From::Multiple {
                    from,
                    sender: u.arbitrary()?,
                })
            }
            2 => Ok(From::InvalidMissingFrom {
                sender: u.arbitrary()?,
            }),
            3 => {
                let mut from: MailboxList = u.arbitrary()?;
                from.0.push(u.arbitrary()?);
                Ok(From::InvalidMissingSender { from })
            }
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, ContainsUtf8, Debug, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary, FuzzEq))]
pub enum TraceField<'a> {
    // At the moment, we do not try to parse the structure of Received fields.
    // RFC5322 gives a rough grammar for tokenizing these fields, relegating
    // their actual interpretation to RFC5321 (which is, for now, outside of
    // the scope of this library).
    // Furthermore, in practice many real-world emails contain Received headers
    // that do not parse even wrt to the rough tokenization of RFC5322.
    Received(Unstructured<'a>),
    ReturnPath(ReturnPath<'a>),
}

impl<'a> Imf<'a> {
    pub fn new() -> Self {
        Self {
            date: DateTimeOpt::InvalidMissing,
            from: From::InvalidMissingFrom { sender: None },
            reply_to: vec![],
            to: vec![],
            cc: vec![],
            bcc: None,
            msg_id: None,
            in_reply_to: vec![],
            references: vec![],
            subject: None,
            comments: vec![],
            keywords: vec![],
            trace: vec![],
            mime_version: None,
        }
    }

    pub fn from_or_sender(&self) -> Option<&MailboxRef<'a>> {
        match &self.from {
            From::Single {
                from: _,
                sender: Some(sender),
            } => Some(sender),
            From::Single { from, sender: None } => Some(from),
            From::Multiple { from: _, sender } => Some(sender),
            From::InvalidMissingFrom { sender } => sender.as_ref(),
            From::InvalidMissingSender { from: _ } => None,
        }
    }

    pub fn from(&self) -> Option<MailboxList<'a>> {
        match &self.from {
            From::Single { from, .. } => Some(MailboxList(vec![from.clone()])),
            From::Multiple { from, .. } => Some(from.clone()),
            From::InvalidMissingFrom { sender: _ } => None,
            From::InvalidMissingSender { from } => Some(from.clone()),
        }
    }

    pub fn sender(&self) -> Option<MailboxRef<'a>> {
        match &self.from {
            From::Single { sender, .. } => sender.clone(),
            From::Multiple { sender, .. } => Some(sender.clone()),
            From::InvalidMissingFrom { sender } => sender.clone(),
            From::InvalidMissingSender { from: _ } => None,
        }
    }

    pub fn get_field(&self, f: field::Entry) -> Option<field::Field<'a>> {
        match f {
            field::Entry::Date => match &self.date {
                DateTimeOpt::Some(d) => Some(field::Field::Date(d.clone())),
                DateTimeOpt::InvalidMissing => None,
            },
            field::Entry::From => self.from().map(field::Field::From),
            field::Entry::Sender => self.sender().map(field::Field::Sender),
            field::Entry::ReplyTo => {
                if !self.reply_to.is_empty() {
                    Some(field::Field::ReplyTo(self.reply_to.clone()))
                } else {
                    None
                }
            }
            field::Entry::To => {
                if !self.to.is_empty() {
                    Some(field::Field::To(self.to.clone()))
                } else {
                    None
                }
            }
            field::Entry::Cc => {
                if !self.cc.is_empty() {
                    Some(field::Field::Cc(self.cc.clone()))
                } else {
                    None
                }
            }
            field::Entry::Bcc => self.bcc.clone().map(field::Field::Bcc),
            field::Entry::MessageID => self.msg_id.clone().map(field::Field::MessageID),
            field::Entry::InReplyTo => {
                if !self.in_reply_to.is_empty() {
                    Some(field::Field::InReplyTo(self.in_reply_to.clone()))
                } else {
                    None
                }
            }
            field::Entry::References => {
                if !self.references.is_empty() {
                    Some(field::Field::References(self.references.clone()))
                } else {
                    None
                }
            }
            field::Entry::Subject => self.subject.clone().map(field::Field::Subject),
            field::Entry::Comments(i) => Some(field::Field::Comments(self.comments[i].clone())),
            field::Entry::Keywords(i) => Some(field::Field::Keywords(self.keywords[i].clone())),
            field::Entry::MIMEVersion => self.mime_version.clone().map(field::Field::MIMEVersion),
            field::Entry::Trace(i) => match &self.trace[i] {
                TraceField::Received(r) => Some(field::Field::Received(r.clone())),
                TraceField::ReturnPath(p) => Some(field::Field::ReturnPath(p.clone())),
            },
        }
    }

    // Returns the entries included in this Imf struct. This is used to define
    // the Arbitrary instance for Message, to construct a randomly ordered list
    // of field entries.
    //
    // The first component of the pair is the list of trace entries (for which
    // the order matters), and the second component is the set of other entries.
    pub fn field_entries(&self) -> (Vec<field::Entry>, HashSet<field::Entry>) {
        let mut trace = vec![];
        for i in 0..self.trace.len() {
            trace.push(field::Entry::Trace(i))
        }

        let mut fs = HashSet::default();
        if let DateTimeOpt::Some(_) = &self.date {
            fs.insert(field::Entry::Date);
        }
        match &self.from {
            From::Single { from: _, sender } => {
                fs.insert(field::Entry::From);
                if sender.is_some() {
                    fs.insert(field::Entry::Sender);
                }
            }
            From::Multiple { from: _, sender: _ } => {
                fs.insert(field::Entry::From);
                fs.insert(field::Entry::Sender);
            }
            From::InvalidMissingFrom { sender } => {
                if sender.is_some() {
                    fs.insert(field::Entry::Sender);
                }
            }
            From::InvalidMissingSender { from: _ } => {
                fs.insert(field::Entry::From);
            }
        }
        if !self.reply_to.is_empty() {
            fs.insert(field::Entry::ReplyTo);
        }
        if !self.to.is_empty() {
            fs.insert(field::Entry::To);
        }
        if !self.cc.is_empty() {
            fs.insert(field::Entry::Cc);
        }
        if self.bcc.is_some() {
            fs.insert(field::Entry::Bcc);
        }
        if self.msg_id.is_some() {
            fs.insert(field::Entry::MessageID);
        }
        if !self.in_reply_to.is_empty() {
            fs.insert(field::Entry::InReplyTo);
        }
        if !self.references.is_empty() {
            fs.insert(field::Entry::References);
        }
        if self.subject.is_some() {
            fs.insert(field::Entry::Subject);
        }
        for i in 0..self.comments.len() {
            fs.insert(field::Entry::Comments(i));
        }
        for i in 0..self.keywords.len() {
            fs.insert(field::Entry::Keywords(i));
        }
        for i in 0..self.keywords.len() {
            fs.insert(field::Entry::Keywords(i));
        }
        fs.insert(field::Entry::MIMEVersion);

        (trace, fs)
    }
}

impl<'a> Default for Imf<'a> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Default, PartialEq, ToStatic)]
pub struct PartialImf<'a> {
    date: Option<DateTime>,
    from: Option<MailboxList<'a>>,
    sender: Option<MailboxRef<'a>>,
    reply_to: Option<Vec<AddressRef<'a>>>,
    to: Option<Vec<AddressRef<'a>>>,
    cc: Option<Vec<AddressRef<'a>>>,
    bcc: Option<Vec<AddressRef<'a>>>,
    msg_id: Option<MessageID<'a>>,
    in_reply_to: Option<Vec<MessageID<'a>>>,
    references: Option<Vec<MessageID<'a>>>,
    subject: Option<Unstructured<'a>>,
    comments: Vec<Unstructured<'a>>,
    keywords: Vec<PhraseList<'a>>,
    trace: Vec<TraceField<'a>>,
    trace_complete: bool,
    mime_version: Option<Version>,
}

#[derive(Clone, Copy, Debug)]
pub enum AddFieldErr {
    // This field results in no entry, but its contents have been taken into
    // account and there is no loss of data.
    NoEntry,
    // This field is conflicting with an earlier field and must be dropped, its
    // data will not be part of the IMF AST.
    Conflict,
}

impl<'a> PartialImf<'a> {
    pub fn add_field(&mut self, f: Field<'a>) -> Result<Entry, AddFieldErr> {
        match &f {
            // trace fields
            Field::ReturnPath(_) | Field::Received(_) => {
                if self.trace_complete {
                    // drop trace fields that come after other IMF fields
                    return Err(AddFieldErr::Conflict);
                }
            }
            // non-trace fields
            _ => {
                // register the trace section to be complete as soon as
                // we encounter a non-trace field
                self.trace_complete = true;
            }
        }
        match f {
            Field::Date(date) => set_if_new(&mut self.date, date, Entry::Date),
            Field::From(from) => set_if_new(&mut self.from, from, Entry::From),
            Field::Sender(sender) => set_if_new(&mut self.sender, sender, Entry::Sender),
            Field::ReplyTo(reply_to) => set_if_new(&mut self.reply_to, reply_to, Entry::ReplyTo),
            Field::To(to) => set_or_extend(&mut self.to, to, Entry::To),
            Field::Cc(cc) => set_or_extend(&mut self.cc, cc, Entry::Cc),
            Field::Bcc(bcc) => set_or_extend(&mut self.bcc, bcc, Entry::Bcc),
            Field::MessageID(id) => set_if_new(&mut self.msg_id, id, Entry::MessageID),
            Field::InReplyTo(in_reply_to) => {
                set_if_new(&mut self.in_reply_to, in_reply_to, Entry::InReplyTo)
            }
            Field::References(refs) => set_if_new(&mut self.references, refs, Entry::References),
            Field::Subject(subject) => set_if_new(&mut self.subject, subject, Entry::Subject),
            Field::Comments(comments) => {
                let idx = self.comments.len();
                self.comments.push(comments);
                Ok(Entry::Comments(idx))
            }
            Field::Keywords(kwds) => {
                let idx = self.keywords.len();
                self.keywords.push(kwds);
                Ok(Entry::Keywords(idx))
            }
            Field::Received(received) => {
                let idx = self.trace.len();
                self.trace.push(TraceField::Received(received));
                Ok(Entry::Trace(idx))
            }
            Field::ReturnPath(path) => {
                let idx = self.trace.len();
                self.trace.push(TraceField::ReturnPath(path));
                Ok(Entry::Trace(idx))
            }
            Field::MIMEVersion(version) => {
                set_if_new(&mut self.mime_version, version, Entry::MIMEVersion)
            }
        }
    }

    pub fn to_imf(self) -> Imf<'a> {
        let date = match self.date {
            Some(dt) => DateTimeOpt::Some(dt),
            None => DateTimeOpt::InvalidMissing,
        };
        let from = match (self.from, self.sender) {
            (None, sender) => From::InvalidMissingFrom { sender },
            (Some(mut l), sender) if l.0.len() == 1 => From::Single {
                from: l.0.pop().unwrap(),
                sender,
            },
            (Some(l), Some(sender)) => From::Multiple { from: l, sender },
            (Some(l), None) => From::InvalidMissingSender { from: l },
        };

        Imf {
            date,
            from,
            reply_to: self.reply_to.unwrap_or_default(),
            to: self.to.unwrap_or_default(),
            cc: self.cc.unwrap_or_default(),
            bcc: self.bcc,
            msg_id: self.msg_id,
            in_reply_to: self.in_reply_to.unwrap_or_default(),
            references: self.references.unwrap_or_default(),
            subject: self.subject,
            comments: self.comments,
            keywords: self.keywords,
            trace: self.trace,
            mime_version: self.mime_version,
        }
    }
}

fn set_if_new<T: PartialEq, U>(o: &mut Option<T>, x: T, y: U) -> Result<U, AddFieldErr> {
    match *o {
        None => {
            *o = Some(x);
            Ok(y)
        }
        Some(_) => Err(AddFieldErr::Conflict),
    }
}

fn set_or_extend<T, U>(o: &mut Option<Vec<T>>, x: Vec<T>, y: U) -> Result<U, AddFieldErr> {
    match o {
        None => {
            *o = Some(x);
            Ok(y)
        }
        Some(v) => {
            v.extend(x);
            Err(AddFieldErr::NoEntry)
        }
    }
}
