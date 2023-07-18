use crate::error::IMFError;
use crate::fragments::lazy::{Field as Lazy, MIMEField as LazyMIME};
use crate::fragments::mime::{Mechanism, Type, Version};
use crate::fragments::misc_token::{PhraseList, Unstructured};
use crate::fragments::model::{AddressList, MailboxList, MailboxRef, MessageId, MessageIdList};
use crate::fragments::trace::ReceivedLog;
use chrono::{DateTime, FixedOffset};

#[derive(Debug, PartialEq)]
pub enum Field<'a> {
    // 3.6.1.  The Origination Date Field
    Date(DateTime<FixedOffset>),

    // 3.6.2.  Originator Fields
    From(MailboxList),
    Sender(MailboxRef),
    ReplyTo(AddressList),

    // 3.6.3.  Destination Address Fields
    To(AddressList),
    Cc(AddressList),
    Bcc(AddressList),

    // 3.6.4.  Identification Fields
    MessageID(MessageId<'a>),
    InReplyTo(MessageIdList<'a>),
    References(MessageIdList<'a>),

    // 3.6.5.  Informational Fields
    Subject(Unstructured),
    Comments(Unstructured),
    Keywords(PhraseList),

    // 3.6.6   Resent Fields (not implemented)
    // 3.6.7   Trace Fields
    Received(ReceivedLog<'a>),
    ReturnPath(MailboxRef),

    // MIME RFC2045
    MIMEVersion(Version),
    MIME(MIMEField<'a>),

    // 3.6.8.  Optional Fields
    Optional(&'a str, Unstructured),

    // None
    Rescue(&'a str),
}

#[derive(Debug, PartialEq)]
pub enum MIMEField<'a> {
    ContentType(Type<'a>),
    ContentTransferEncoding(Mechanism<'a>),
    ContentID(MessageId<'a>),
    ContentDescription(Unstructured),
    Optional(&'a str, Unstructured),
    Rescue(&'a str),
}

impl<'a> TryFrom<&'a Lazy<'a>> for Field<'a> {
    type Error = IMFError<'a>;

    fn try_from(l: &'a Lazy<'a>) -> Result<Self, Self::Error> {
        use Field::*;
        match l {
            Lazy::Date(v) => v.try_into().map(|v| Date(v)),
            Lazy::From(v) => v.try_into().map(|v| From(v)),
            Lazy::Sender(v) => v.try_into().map(|v| Sender(v)),
            Lazy::ReplyTo(v) => v.try_into().map(|v| ReplyTo(v)),
            Lazy::To(v) => v.try_into().map(|v| To(v)),
            Lazy::Cc(v) => v.try_into().map(|v| Cc(v)),
            Lazy::Bcc(v) => v.try_into().map(|v| Bcc(v)),
            Lazy::MessageID(v) => v.try_into().map(|v| MessageID(v)),
            Lazy::InReplyTo(v) => v.try_into().map(|v| InReplyTo(v)),
            Lazy::References(v) => v.try_into().map(|v| References(v)),
            Lazy::Subject(v) => v.try_into().map(|v| Subject(v)),
            Lazy::Comments(v) => v.try_into().map(|v| Comments(v)),
            Lazy::Keywords(v) => v.try_into().map(|v| Keywords(v)),
            Lazy::Received(v) => v.try_into().map(|v| Received(v)),
            Lazy::ReturnPath(v) => v.try_into().map(|v| ReturnPath(v)),
            Lazy::MIMEVersion(v) => v.try_into().map(|v| MIMEVersion(v)),
            Lazy::MIME(v) => v.try_into().map(|v| MIME(v)),
            Lazy::Optional(k, v) => v.try_into().map(|v| Optional(k, v)),
            Lazy::Rescue(v) => Ok(Rescue(v)),
        }
    }
}

impl<'a> TryFrom<&'a LazyMIME<'a>> for MIMEField<'a> {
    type Error = IMFError<'a>;

    fn try_from(l: &'a LazyMIME<'a>) -> Result<Self, Self::Error> {
        use MIMEField::*;
        match l {
            LazyMIME::ContentType(v) => v.try_into().map(|v| ContentType(v)),
            LazyMIME::ContentTransferEncoding(v) => v.try_into().map(|v| ContentTransferEncoding(v)),
            LazyMIME::ContentID(v) => v.try_into().map(|v| ContentID(v)),
            LazyMIME::ContentDescription(v) => v.try_into().map(|v| ContentDescription(v)),
            LazyMIME::Optional(k, v) => v.try_into().map(|v| Optional(k, v)),
            LazyMIME::Rescue(v) => Ok(Rescue(v)),
        }
    }
}
