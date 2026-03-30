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
use crate::header;
use crate::imf::address::AddressRef;
use crate::imf::datetime::DateTime;
use crate::imf::field::{Field, Entry};
use crate::imf::identification::MessageID;
use crate::imf::mailbox::{MailboxRef, MailboxList};
use crate::imf::mime::Version;
use crate::imf::trace::ReturnPath;
use crate::print::Formatter;
use crate::text::misc_token::{PhraseList, Unstructured};

#[derive(Clone, Debug, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub struct Imf<'a> {
    // 3.6.1.  The Origination Date Field
    pub date: DateTime,

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
    pub mime_version: Version,

    // This field is for information only, and should not be considered part of
    // the "structured" Imf AST. It contains fields encountered during parsing
    // that were discarded because they conflicted with an earlier field (e.g.
    // because there were multiple occurrences of a field that must only appear
    // once). These discarded fields are never printed back.
    #[cfg_attr(feature = "arbitrary", fuzz_eq(ignore))]
    pub discarded: Vec<field::Field<'a>>,
}

#[derive(Clone, Debug, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub enum From<'a> {
    Single {
        from: MailboxRef<'a>,
        sender: Option<MailboxRef<'a>>,
    },
    Multiple {
        from: MailboxList<'a>, // must contain at least two elements
        sender: MailboxRef<'a>,
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for Imf<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        // XXX it would be nicer if the Arbitrary derive macro supported field
        // attributes to allow setting a value for `discarded`, rather than
        // having to write this implementation by hand.
        Ok(Self {
            date: u.arbitrary()?,
            from: u.arbitrary()?,
            reply_to: u.arbitrary()?,
            to: u.arbitrary()?,
            cc: u.arbitrary()?,
            bcc: u.arbitrary()?,
            msg_id: u.arbitrary()?,
            in_reply_to: u.arbitrary()?,
            references: u.arbitrary()?,
            subject: u.arbitrary()?,
            comments: u.arbitrary()?,
            keywords: u.arbitrary()?,
            trace: u.arbitrary()?,
            mime_version: u.arbitrary()?,
            discarded: vec![],
        })
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for From<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        if u.arbitrary()? {
            Ok(From::Single {
                from: u.arbitrary()?,
                sender: u.arbitrary()?,
            })
        } else {
            let mut from: MailboxList = u.arbitrary()?;
            from.0.push(u.arbitrary()?);
            Ok(From::Multiple {
                from,
                sender: u.arbitrary()?,
            })
        }
    }
}

#[derive(Clone, Debug, PartialEq, ToStatic)]
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
    pub fn new(from: From<'a>, date: DateTime) -> Imf<'a> {
        Imf {
            date,
            from,
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
            mime_version: Version::default(),
            discarded: vec![],
        }
    }

    pub fn from_or_sender(&self) -> &MailboxRef<'a> {
        match &self.from {
            From::Single { from: _, sender: Some(sender) } => sender,
            From::Single { from, sender: None } => from,
            From::Multiple { from: _, sender } => sender,
        }
    }

    pub fn from(&self) -> MailboxList<'a> {
        match &self.from {
            From::Single { from, .. } => MailboxList(vec![from.clone()]),
            From::Multiple { from, .. } => from.clone(),
        }
    }

    pub fn sender(&self) -> Option<MailboxRef<'a>> {
        match &self.from {
            From::Single { sender, .. } => sender.clone(),
            From::Multiple { sender, .. } => Some(sender.clone()),
        }
    }

    pub fn print_field(&self, f: field::Entry, fmt: &mut impl Formatter) {
        match f {
            field::Entry::Date =>
                header::print(fmt, b"Date", &self.date),
            field::Entry::From =>
                header::print(fmt, b"From", self.from()),
            field::Entry::Sender => {
                if let Some(sender) = self.sender() {
                    header::print(fmt, b"Sender", sender)
                }
            },
            field::Entry::ReplyTo => {
                if !self.reply_to.is_empty() {
                    header::print(fmt, b"Reply-To", &self.reply_to)
                }
            },
            field::Entry::To => {
                if !self.to.is_empty() {
                    header::print(fmt, b"To", &self.to)
                }
            },
            field::Entry::Cc => {
                if !self.cc.is_empty() {
                    header::print(fmt, b"Cc", &self.cc)
                }
            },
            field::Entry::Bcc => {
                if let Some(bcc) = &self.bcc {
                    header::print(fmt, b"Bcc", bcc)
                }
            },
            field::Entry::MessageId => {
                if let Some(msg_id) = &self.msg_id {
                    header::print(fmt, b"Message-ID", msg_id)
                }
            },
            field::Entry::InReplyTo => {
                if !self.in_reply_to.is_empty() {
                    header::print(fmt, b"In-Reply-To", &self.in_reply_to)
                }
            },
            field::Entry::References => {
                if !self.references.is_empty() {
                    header::print(fmt, b"References", &self.references)
                }
            },
            field::Entry::Subject => {
                if let Some(subject) = &self.subject {
                    header::print_unstructured(fmt, b"Subject", subject)
                }
            },
            field::Entry::Comments(i) =>
                header::print_unstructured(fmt, b"Comments", &self.comments[i]),
            field::Entry::Keywords(i) =>
                header::print(fmt, b"Keywords", &self.keywords[i]),
            field::Entry::MIMEVersion =>
                header::print(fmt, b"MIME-Version", &self.mime_version),
            field::Entry::Trace(i) =>
                match &self.trace[i] {
                    TraceField::Received(r) =>
                        header::print_unstructured(fmt, b"Received", r),
                    TraceField::ReturnPath(p) =>
                        header::print(fmt, b"Return-Path", p),
                }
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
        fs.insert(field::Entry::Date);
        fs.insert(field::Entry::From);
        match &self.from {
            From::Single { from: _, sender } => {
                if sender.is_some() {
                    fs.insert(field::Entry::Sender);
                }
            },
            From::Multiple { from: _, sender: _ } => {
                fs.insert(field::Entry::Sender);
            },
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
            fs.insert(field::Entry::MessageId);
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
    discarded: Vec<field::Field<'a>>,
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
        // XXX it is a bit unfortunate to have to .clone() here just
        // because of the AddFieldErr::Conflict case below
        let res = self.add_field_inner(f.clone());

        // Store dropped fields in `self.discarded`, for information
        // purposes only.
        if let Err(AddFieldErr::Conflict) = res {
            self.discarded.push(f);
        }

        res
    }

    fn add_field_inner(&mut self, f: Field<'a>) -> Result<Entry, AddFieldErr> {
        match &f {
            // trace fields
            Field::ReturnPath(_) |
            Field::Received(_) => {
                if self.trace_complete {
                    // drop trace fields that come after other IMF fields
                    return Err(AddFieldErr::Conflict)
                }
            },
            // non-trace fields
            _ => {
                // register the trace section to be complete as soon as
                // we encounter a non-trace field
                self.trace_complete = true;
            }
        }
        match f {
            Field::Date(date) =>
                set_if_new(&mut self.date, date, Entry::Date),
            Field::From(from) =>
                set_if_new(&mut self.from, from, Entry::From),
            Field::Sender(sender) =>
                set_if_new(&mut self.sender, sender, Entry::Sender),
            Field::ReplyTo(reply_to) =>
                set_if_new(&mut self.reply_to, reply_to, Entry::ReplyTo),
            Field::To(to) =>
                set_or_extend(&mut self.to, to, Entry::To),
            Field::Cc(cc) =>
                set_or_extend(&mut self.cc, cc, Entry::Cc),
            Field::Bcc(bcc) =>
                set_or_extend(&mut self.bcc, bcc, Entry::Bcc),
            Field::MessageID(id) =>
                set_if_new(&mut self.msg_id, id, Entry::MessageId),
            Field::InReplyTo(in_reply_to) =>
                set_if_new(&mut self.in_reply_to, in_reply_to, Entry::InReplyTo),
            Field::References(refs) =>
                set_if_new(&mut self.references, refs, Entry::References),
            Field::Subject(subject) =>
                set_if_new(&mut self.subject, subject, Entry::Subject),
            Field::Comments(comments) => {
                let idx = self.comments.len();
                self.comments.push(comments);
                Ok(Entry::Comments(idx))
            },
            Field::Keywords(kwds) => {
                let idx = self.keywords.len();
                self.keywords.push(kwds);
                Ok(Entry::Keywords(idx))
            },
            Field::Received(received) => {
                let idx = self.trace.len();
                self.trace.push(TraceField::Received(received));
                Ok(Entry::Trace(idx))
            },
            Field::ReturnPath(path) => {
                let idx = self.trace.len();
                self.trace.push(TraceField::ReturnPath(path));
                Ok(Entry::Trace(idx))
            },
            Field::MIMEVersion(version) =>
                set_if_new(&mut self.mime_version, version, Entry::MIMEVersion),
        }
    }

    pub fn missing_mandatory_fields(&self) -> Vec<Entry> {
        let mut entries = Vec::new();
        if self.date.is_none() {
            entries.push(Entry::Date)
        }
        match &self.from {
            None => {
                entries.push(Entry::From)
            },
            Some(v) => {
                if v.0.is_empty() {
                    entries.push(Entry::From)
                } else if v.0.len() > 1 && self.sender.is_none() {
                    entries.push(Entry::Sender)
                }
            },
        }
        if self.mime_version.is_none() {
            entries.push(Entry::MIMEVersion)
        }
        entries
    }

    pub fn to_imf(self) -> Imf<'a> {
        let date = self.date.unwrap_or_else(DateTime::placeholder);
        let from = {
            let mut p_from = self.from.unwrap_or_else(|| MailboxList(vec![MailboxRef::placeholder()]));
            if p_from.0.len() == 1 {
                From::Single {
                    from: p_from.0.pop().unwrap(),
                    sender: self.sender,
                }
            } else {
                From::Multiple {
                    from: p_from,
                    sender: self.sender.unwrap_or_else(|| MailboxRef::placeholder()),
                }
            }
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
            // XXX we don't support reading non-MIME compliant emails
            // currently, so we always turn a missing MIME-Version field
            // into the default supported version
            mime_version: self.mime_version.unwrap_or_default(),
            discarded: self.discarded,
        }
    }
}

fn set_if_new<T: PartialEq, U>(o: &mut Option<T>, x: T, y: U) -> Result<U, AddFieldErr> {
    match *o {
        None => { *o = Some(x); Ok(y) },
        Some(_) => Err(AddFieldErr::Conflict),
    }
}

fn set_or_extend<T, U>(o: &mut Option<Vec<T>>, x: Vec<T>, y: U) -> Result<U, AddFieldErr> {
    match o {
        None => { *o = Some(x); Ok(y) },
        Some(v) => { v.extend(x); Err(AddFieldErr::NoEntry) },
    }
}
