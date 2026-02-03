/// Parse and represent IMF (Internet Message Format) headers (RFC822, RFC5322)
pub mod address;
pub mod datetime;
pub mod field;
pub mod identification;
pub mod mailbox;
pub mod mime;
pub mod trace;

use bounded_static::ToStatic;

use crate::header;
use crate::imf::address::AddressRef;
use crate::imf::datetime::DateTime;
use crate::imf::field::{Field, Entry};
use crate::imf::identification::MessageID;
use crate::imf::mailbox::{MailboxRef, MailboxList};
use crate::imf::mime::Version;
use crate::imf::trace::{ReceivedLog, ReturnPath, TraceBlock};
use crate::print::Formatter;
use crate::text::misc_token::{PhraseList, Unstructured};
use crate::utils::{append_opt, set_opt};

#[derive(Clone, Debug, PartialEq, ToStatic)]
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
    pub trace: Vec<TraceBlock<'a>>,

    // MIME
    pub mime_version: Version,
}

#[derive(Clone, Debug, PartialEq, ToStatic)]
pub enum From<'a> {
    Single {
        from: MailboxRef<'a>,
        sender: Option<MailboxRef<'a>>,
    },
    Multiple {
        from: MailboxList<'a>,
        sender: MailboxRef<'a>,
    }
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
            field::Entry::ReturnPath(i) =>
                header::print(fmt, b"Return-Path", &self.trace[i].return_path.as_ref().unwrap()),
            field::Entry::Received(i, j) =>
                header::print(fmt, b"Received", &self.trace[i].received[j]),
            field::Entry::MIMEVersion =>
                header::print(fmt, b"MIME-Version", &self.mime_version),
        }
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
    trace: Vec<PartialTraceBlock<'a>>,
    trace_complete: bool,
    mime_version: Option<Version>,
}

#[derive(Debug, Default, PartialEq, ToStatic)]
pub struct PartialTraceBlock<'a> {
    return_path: Option<ReturnPath<'a>>,
    received: Vec<ReceivedLog<'a>>,
}

impl<'a> PartialImf<'a> {
    pub fn add_field(&mut self, f: Field<'a>) -> Option<Entry> {
        match &f {
            // trace fields
            Field::ReturnPath(_) |
            Field::Received(_) => {
                if self.trace_complete {
                    // drop trace fields that are not at the beginning
                    return None
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
            Field::Date(date) => {
                if set_opt(&mut self.date, date) {
                    return Some(Entry::Date)
                }
            },
            Field::From(from) => {
                if set_opt(&mut self.from, from) {
                    return Some(Entry::From)
                }
            },
            Field::Sender(sender) => {
                if set_opt(&mut self.sender, sender) {
                    return Some(Entry::Sender)
                }
            },
            Field::ReplyTo(reply_to) => {
                if set_opt(&mut self.reply_to, reply_to) {
                    return Some(Entry::ReplyTo)
                }
            },
            Field::To(to) => {
                if append_opt(&mut self.to, to) {
                    return Some(Entry::To)
                }
            },
            Field::Cc(cc) => {
                if append_opt(&mut self.cc, cc) {
                    return Some(Entry::Cc)
                }
            },
            Field::Bcc(bcc) => {
                if append_opt(&mut self.bcc, bcc) {
                    return Some(Entry::Bcc)
                }
            },
            Field::MessageID(id) => {
                if set_opt(&mut self.msg_id, id) {
                    return Some(Entry::MessageId)
                }
            },
            Field::InReplyTo(in_reply_to) => {
                if set_opt(&mut self.in_reply_to, in_reply_to) {
                    return Some(Entry::InReplyTo)
                }
            },
            Field::References(refs) => {
                if set_opt(&mut self.references, refs) {
                    return Some(Entry::References)
                }
            },
            Field::Subject(subject) => {
                if set_opt(&mut self.subject, subject) {
                    return Some(Entry::Subject)
                }
            },
            Field::Comments(comments) => {
                let idx = self.comments.len();
                self.comments.push(comments);
                return Some(Entry::Comments(idx))
            },
            Field::Keywords(kwds) => {
                // the obs syntax allows empty phrase lists, but not
                // the normal syntax. we drop them.
                if let Some(kwds) = kwds {
                    let idx = self.keywords.len();
                    self.keywords.push(kwds);
                    return Some(Entry::Keywords(idx))
                }
            },
            Field::Received(received) => {
                if self.trace.is_empty() {
                    self.trace.push(PartialTraceBlock::default())
                }
                let block_idx = self.trace.len() - 1;
                let field_idx = self.trace[block_idx].received.len();
                self.trace[block_idx].received.push(received);
                return Some(Entry::Received(block_idx, field_idx))
            },
            Field::ReturnPath(path) => {
                let block_idx = self.trace.len();
                self.trace.push(PartialTraceBlock {
                    return_path: Some(path),
                    received: vec![],
                });
                return Some(Entry::ReturnPath(block_idx))
            },
            Field::MIMEVersion(version) => {
                if set_opt(&mut self.mime_version, version) {
                    return Some(Entry::MIMEVersion)
                }
            }
        };
        None
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
        let trace = {
            // drop trace blocks with zero 'received' as they are not allowed
            self.trace.into_iter().filter_map(|PartialTraceBlock { return_path, received }| {
                if received.is_empty() {
                    None
                } else {
                    Some(TraceBlock { return_path, received })
                }
            }).collect()
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
            trace,
            // XXX we don't support reading non-MIME compliant emails
            // currently, so we always turn a missing MIME-Version field
            // into the default supported version
            mime_version: self.mime_version.unwrap_or_default()
        }
    }
}
