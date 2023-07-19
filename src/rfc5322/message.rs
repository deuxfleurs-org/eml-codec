use std::collections::HashMap;

use crate::fragments::eager::{Field, MIMEField};
use crate::fragments::lazy;
use crate::fragments::misc_token::{PhraseList, Unstructured};
use crate::fragments::mime::{Version,Type,Mechanism};
use crate::fragments::model::{AddressRef, MailboxRef, MessageId};
use crate::fragments::trace::ReceivedLog;
use chrono::{DateTime, FixedOffset};

#[derive(Debug, PartialEq, Default)]
pub struct Message<'a> {
    // 3.6.1.  The Origination Date Field
    pub date: Option<DateTime<FixedOffset>>,

    // 3.6.2.  Originator Fields
    pub from: Vec<MailboxRef<'a>>,
    pub sender: Option<MailboxRef<'a>>,
    pub reply_to: Vec<AddressRef<'a>>,

    // 3.6.3.  Destination Address Fields
    pub to: Vec<AddressRef<'a>>,
    pub cc: Vec<AddressRef<'a>>,
    pub bcc: Vec<AddressRef<'a>>,

    // 3.6.4.  Identification Fields
    pub msg_id: Option<MessageId<'a>>,
    pub in_reply_to: Vec<MessageId<'a>>,
    pub references: Vec<MessageId<'a>>,

    // 3.6.5.  Informational Fields
    pub subject: Option<Unstructured<'a>>,
    pub comments: Vec<Unstructured<'a>>,
    pub keywords: Vec<PhraseList<'a>>,

    // 3.6.6 Not implemented
    // 3.6.7 Trace Fields
    pub return_path: Vec<MailboxRef<'a>>,
    pub received: Vec<ReceivedLog<'a>>,

    // 3.6.8.  Optional Fields
    pub optional: HashMap<&'a [u8], Unstructured<'a>>,

    // Recovery
    pub unparsed: Vec<&'a [u8]>,
}

impl<'a> FromIterator<&'a [u8]> for Message<'a> {
    fn from_iter<I: IntoIterator<Item = &'a [u8]>>(iter: I) -> Self {
        iter.fold(
            Message::default(),
            |mut msg, field| {
                match field_name(field) {
                    Ok((name, value)) => xx,

                }

                match field {

                }
                msg
            }
        )
    }
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
                Field::MIMEVersion(v) => section.mime_version = Some(v),
                Field::MIME(v) => match v {
                    MIMEField::ContentType(v) => section.mime.content_type = Some(v),
                    MIMEField::ContentTransferEncoding(v) => section.mime.content_transfer_encoding = Some(v),
                    MIMEField::ContentID(v) => section.mime.content_id = Some(v),
                    MIMEField::ContentDescription(v) => section.mime.content_description = Some(v),
                    MIMEField::Optional(k, v) => {
                        section.mime.optional.insert(k, v);
                    }
                    MIMEField::Rescue(v) => section.mime.unparsed.push(v),

                },
            }
        }
        section
    }
}

