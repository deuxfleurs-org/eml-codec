use std::collections::HashMap;

use crate::text::misc_token::{PhraseList, Unstructured};
use crate::rfc5322::mime::Version;
use crate::rfc5322::mailbox::{MailboxRef};
use crate::rfc5322::address::{AddressRef};
use crate::rfc5322::identification::{MessageID, MessageIDList};
use crate::rfc5322::field::Field;
use crate::rfc5322::trace::ReceivedLog;
use chrono::{DateTime, FixedOffset};

#[derive(Debug, PartialEq, Default)]
pub struct Message<'a> {
    // 3.6.1.  The Origination Date Field
    pub date: &'a Option<DateTime<FixedOffset>>,

    // 3.6.2.  Originator Fields
    pub from: Vec<&'a MailboxRef<'a>>,
    pub sender: Option<&'a MailboxRef<'a>>,
    pub reply_to: Vec<&'a AddressRef<'a>>,

    // 3.6.3.  Destination Address Fields
    pub to: Vec<&'a AddressRef<'a>>,
    pub cc: Vec<&'a AddressRef<'a>>,
    pub bcc: Vec<&'a AddressRef<'a>>,

    // 3.6.4.  Identification Fields
    pub msg_id: Option<&'a MessageID<'a>>,
    pub in_reply_to: MessageIDList<'a>,
    pub references: MessageIDList<'a>,

    // 3.6.5.  Informational Fields
    pub subject: Option<&'a Unstructured<'a>>,
    pub comments: Vec<&'a Unstructured<'a>>,
    pub keywords: Vec<&'a PhraseList<'a>>,

    // 3.6.6 Not implemented
    // 3.6.7 Trace Fields
    pub return_path: Vec<&'a MailboxRef<'a>>,
    pub received: Vec<&'a ReceivedLog<'a>>,

    // MIME
    pub mime_version: Option<&'a Version>,
}

//@FIXME min and max limits are not enforced,
// it may result in missing data or silently overriden data.
impl<'a> FromIterator<&'a Field<'a>> for Message<'a> {
    fn from_iter<I: IntoIterator<Item = &'a Field<'a>>>(iter: I) -> Self {
        iter.into_iter().fold(
            Message::default(),
            |mut section, field| {
                match field {
                    Field::Date(v) => section.date = v,
                    Field::From(v) => section.from.extend(v),
                    Field::Sender(v) => section.sender = Some(v),
                    Field::ReplyTo(v) => section.reply_to.extend(v),
                    Field::To(v) => section.to.extend(v),
                    Field::Cc(v) => section.cc.extend(v),
                    Field::Bcc(v) => section.bcc.extend(v),
                    Field::MessageID(v) => section.msg_id = Some(v),
                    Field::InReplyTo(v) => section.in_reply_to.extend(v),
                    Field::References(v) => section.references.extend(v),
                    Field::Subject(v) => section.subject = Some(v),
                    Field::Comments(v) => section.comments.push(v),
                    Field::Keywords(v) => section.keywords.push(v),
                    Field::ReturnPath(v) => section.return_path.push(v),
                    Field::Received(v) => section.received.push(v),
                    Field::MIMEVersion(v) => section.mime_version = Some(v),
                };
                section
            }
        )
    }
}

