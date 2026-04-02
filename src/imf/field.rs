use bounded_static::ToStatic;
use nom::combinator::map;

use crate::header;
use crate::imf::address::{address_list, nullable_address_list, AddressList};
use crate::imf::datetime::{date_time, DateTime};
use crate::imf::identification::{msg_id, msg_list, MessageID, MessageIDList};
use crate::imf::mailbox::{mailbox, mailbox_list, MailboxList, MailboxRef};
use crate::imf::mime::{version, Version};
use crate::imf::trace::{received_log, return_path, ReceivedLog, ReturnPath};
use crate::text::misc_token::{phrase_list, unstructured, PhraseList, Unstructured};

// NOTE: we don't need `Entry` constructors for trace fields
// because they are already stored in-order in the Imf struct.
#[derive(Clone, Copy, Debug, PartialEq, ToStatic)]
pub enum Entry {
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
    MIMEVersion,
}

#[derive(Debug, PartialEq, ToStatic)]
pub enum Field<'a> {
    // 3.6.1.  The Origination Date Field
    Date(DateTime),

    // 3.6.2.  Originator Fields
    From(MailboxList<'a>),
    Sender(MailboxRef<'a>),
    ReplyTo(AddressList<'a>),

    // 3.6.3.  Destination Address Fields
    To(AddressList<'a>),
    Cc(AddressList<'a>),
    Bcc(AddressList<'a>),

    // 3.6.4.  Identification Fields
    MessageID(MessageID<'a>),
    InReplyTo(MessageIDList<'a>),
    References(MessageIDList<'a>),

    // 3.6.5.  Informational Fields
    Subject(Unstructured<'a>),
    Comments(Unstructured<'a>),
    Keywords(Option<PhraseList<'a>>),

    // 3.6.6   Resent Fields (not implemented)
    // 3.6.7   Trace Fields
    Received(ReceivedLog<'a>),
    ReturnPath(ReturnPath<'a>),

    // MIME
    MIMEVersion(Version),
}

pub enum InvalidField {
    Name,
    Body,
}

impl<'a> TryFrom<&header::FieldRaw<'a>> for Field<'a> {
    type Error = InvalidField;
    fn try_from(f: &header::FieldRaw<'a>) -> Result<Self, Self::Error> {
        let content = match f.name.bytes().to_ascii_lowercase().as_slice() {
            b"date" => map(date_time, Field::Date)(f.body),
            b"from" => map(mailbox_list, Field::From)(f.body),
            b"sender" => map(mailbox, Field::Sender)(f.body),
            b"reply-to" => map(address_list, Field::ReplyTo)(f.body),
            b"to" => map(address_list, Field::To)(f.body),
            b"cc" => map(address_list, Field::Cc)(f.body),
            b"bcc" => map(nullable_address_list, Field::Bcc)(f.body),
            b"message-id" => map(msg_id, Field::MessageID)(f.body),
            // TODO: obs-in-reply-to
            b"in-reply-to" => map(msg_list, Field::InReplyTo)(f.body),
            // TODO: obs-references
            b"references" => map(msg_list, Field::References)(f.body),
            b"subject" => map(unstructured, Field::Subject)(f.body),
            b"comments" => map(unstructured, Field::Comments)(f.body),
            b"keywords" => map(phrase_list, Field::Keywords)(f.body),
            b"return-path" => map(return_path, Field::ReturnPath)(f.body),
            b"received" => map(received_log, Field::Received)(f.body),
            b"mime-version" => map(version, Field::MIMEVersion)(f.body),
            _ => return Err(InvalidField::Name),
        };

        // TODO: check that the parser consumed the entire body?
        content.map(|(_, content)| content).or(Err(InvalidField::Body))
    }
}
