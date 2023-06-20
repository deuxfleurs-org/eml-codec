use std::collections::HashMap;

use chrono::{DateTime, FixedOffset};
use crate::fragments::model::{
    MailboxList, MailboxRef, AddressList,
    MessageId, MessageIdList, AddressRef};
use crate::fragments::misc_token::{Unstructured, PhraseList};
use crate::fragments::trace::ReceivedLog;
use crate::fragments::eager::Field;
use crate::fragments::lazy;

#[derive(Debug, PartialEq, Default)]
pub struct Section<'a> {
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
    pub subject: Option<Unstructured>,
    pub comments: Vec<Unstructured>,
    pub keywords: Vec<PhraseList>,

    // 3.6.6 Not implemented
    // 3.6.7 Trace Fields
    pub return_path: Vec<MailboxRef>,
    pub received: Vec<ReceivedLog<'a>>,

    // 3.6.8.  Optional Fields
    pub optional: HashMap<&'a str, Unstructured>,

    // Recovery
    pub bad_fields: Vec<lazy::Field<'a>>,
    pub unparsed: Vec<&'a str>,
}

//@FIXME min and max limits are not enforced,
// it may result in missing data or silently overriden data.
impl<'a> From<Vec<Field<'a>>> for Section<'a> {
    fn from(field_list: Vec<Field<'a>>) -> Self {
        field_list.into_iter().fold(
            Section::default(),
            |mut section, field| {
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
                    Field::Optional(k, v) => { section.optional.insert(k, v); },
                    Field::Rescue(v) => section.unparsed.push(v),
                };
                section
            }
        )
    }
}

