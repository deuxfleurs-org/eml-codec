use std::collections::HashMap;

use crate::fragments::eager::Field;
use crate::fragments::lazy;
use crate::fragments::misc_token::{PhraseList, Unstructured};
use crate::fragments::model::{AddressRef, MailboxRef, MessageId};
use crate::fragments::trace::ReceivedLog;
use chrono::{DateTime, FixedOffset};

#[derive(Debug, PartialEq, Default)]
pub struct Section<'a> {
    // 3.6.1.  The Origination Date Field
    pub date: Option<&'a DateTime<FixedOffset>>,

    // 3.6.2.  Originator Fields
    pub from: Vec<&'a MailboxRef>,
    pub sender: Option<&'a MailboxRef>,
    pub reply_to: Vec<&'a AddressRef>,

    // 3.6.3.  Destination Address Fields
    pub to: Vec<&'a AddressRef>,
    pub cc: Vec<&'a AddressRef>,
    pub bcc: Vec<&'a AddressRef>,

    // 3.6.4.  Identification Fields
    pub msg_id: Option<&'a MessageId<'a>>,
    pub in_reply_to: Vec<&'a MessageId<'a>>,
    pub references: Vec<&'a MessageId<'a>>,

    // 3.6.5.  Informational Fields
    pub subject: Option<&'a Unstructured>,
    pub comments: Vec<&'a Unstructured>,
    pub keywords: Vec<&'a PhraseList>,

    // 3.6.6 Not implemented
    // 3.6.7 Trace Fields
    pub return_path: Vec<&'a MailboxRef>,
    pub received: Vec<&'a ReceivedLog<'a>>,

    // 3.6.8.  Optional Fields
    pub optional: HashMap<&'a str, &'a Unstructured>,

    // Recovery
    pub bad_fields: Vec<&'a lazy::Field<'a>>,
    pub unparsed: Vec<&'a str>,
}

//@FIXME min and max limits are not enforced,
// it may result in missing data or silently overriden data.
impl<'a> FromIterator<&'a Field<'a>> for Section<'a> {
    fn from_iter<I: IntoIterator<Item = &'a Field<'a>>>(iter: I) -> Self {
        let mut section = Section::default();
        for field in iter {
            match field {
                Field::Date(v) => section.date = Some(v),
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
                Field::Optional(k, v) => {
                    section.optional.insert(k, v);
                }
                Field::Rescue(v) => section.unparsed.push(v),
            }
        }
        section
    }
}
