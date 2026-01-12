/// Parse and represent IMF (Internet Message Format) headers (RFC822, RFC5322)
pub mod address;
pub mod datetime;
pub mod field;
pub mod identification;
pub mod mailbox;
pub mod mime;
pub mod trace;

use bounded_static::ToStatic;

use crate::imf::address::AddressRef;
use crate::imf::datetime::DateTime;
use crate::imf::field::Field;
use crate::imf::identification::MessageID;
use crate::imf::mailbox::MailboxRef;
use crate::imf::mime::Version;
use crate::imf::trace::{ReceivedLog, ReturnPath, TraceBlock};
use crate::text::misc_token::{PhraseList, Unstructured};
use crate::utils::{append_opt, set_opt};

#[derive(Debug, PartialEq, ToStatic)]
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

#[derive(Debug, PartialEq, ToStatic)]
pub enum From<'a> {
    Single {
        from: MailboxRef<'a>,
        sender: Option<MailboxRef<'a>>,
    },
    Multiple {
        from: Vec<MailboxRef<'a>>,
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

    pub fn sender(&self) -> &MailboxRef<'a> {
        match &self.from {
            From::Single { from: _, sender: Some(sender) } => sender,
            From::Single { from, sender: None } => from,
            From::Multiple { from: _, sender } => sender,
        }
    }
}

#[derive(Debug, Default, PartialEq, ToStatic)]
pub struct PartialImf<'a> {
    date: Option<DateTime>,
    from: Option<Vec<MailboxRef<'a>>>,
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

#[derive(Clone, Copy, Debug, PartialEq, ToStatic)]
pub enum FieldEntry {
    Date,
    From,
    Sender,
    ReplyTo,
    To,
    Cc,
    Bcc,
    MessageId,
    InReplyTo,
    References,
    Subject,
    Comments(usize),
    Keywords(usize),
    ReturnPath(usize),
    Received(usize, usize),
    MIMEVersion,
}

impl<'a> PartialImf<'a> {
    pub fn add_field(&mut self, f: Field<'a>) -> Option<FieldEntry> {
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
                    return Some(FieldEntry::Date)
                }
            },
            Field::From(from) => {
                if set_opt(&mut self.from, from) {
                    return Some(FieldEntry::From)
                }
            },
            Field::Sender(sender) => {
                if set_opt(&mut self.sender, sender) {
                    return Some(FieldEntry::Sender)
                }
            },
            Field::ReplyTo(reply_to) => {
                if set_opt(&mut self.reply_to, reply_to) {
                    return Some(FieldEntry::ReplyTo)
                }
            },
            Field::To(to) => {
                if append_opt(&mut self.to, to) {
                    return Some(FieldEntry::To)
                }
            },
            Field::Cc(cc) => {
                if append_opt(&mut self.cc, cc) {
                    return Some(FieldEntry::Cc)
                }
            },
            Field::Bcc(bcc) => {
                if append_opt(&mut self.bcc, bcc) {
                    return Some(FieldEntry::Bcc)
                }
            },
            Field::MessageID(id) => {
                if set_opt(&mut self.msg_id, id) {
                    return Some(FieldEntry::MessageId)
                }
            },
            Field::InReplyTo(in_reply_to) => {
                if set_opt(&mut self.in_reply_to, in_reply_to.0) {
                    return Some(FieldEntry::InReplyTo)
                }
            },
            Field::References(refs) => {
                if set_opt(&mut self.references, refs.0) {
                    return Some(FieldEntry::References)
                }
            },
            Field::Subject(subject) => {
                if set_opt(&mut self.subject, subject) {
                    return Some(FieldEntry::Subject)
                }
            },
            Field::Comments(comments) => {
                let idx = self.comments.len();
                self.comments.push(comments);
                return Some(FieldEntry::Comments(idx))
            },
            Field::Keywords(kwds) => {
                let idx = self.keywords.len();
                self.keywords.push(kwds);
                return Some(FieldEntry::Keywords(idx))
            },
            Field::Received(received) => {
                if self.trace.is_empty() {
                    self.trace.push(PartialTraceBlock::default())
                }
                let block_idx = self.trace.len() - 1;
                let field_idx = self.trace[block_idx].received.len();
                self.trace[block_idx].received.push(received);
                return Some(FieldEntry::Received(block_idx, field_idx))
            },
            Field::ReturnPath(path) => {
                let block_idx = self.trace.len();
                self.trace.push(PartialTraceBlock {
                    return_path: Some(path),
                    received: vec![],
                });
                return Some(FieldEntry::ReturnPath(block_idx))
            },
            Field::MIMEVersion(version) => {
                if set_opt(&mut self.mime_version, version) {
                    return Some(FieldEntry::MIMEVersion)
                }
            }
        };
        None
    }

    pub fn to_imf(self) -> Option<Imf<'a>> {
        let date = self.date?;
        let from = {
            let mut p_from = self.from?;
            if p_from.is_empty() {
                return None;
            }
            if p_from.len() == 1 {
                From::Single {
                    from: p_from.pop().unwrap(),
                    sender: self.sender,
                }
            } else {
                From::Multiple {
                    from: p_from,
                    sender: self.sender?,
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

        Some(
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
        )
    }
}
