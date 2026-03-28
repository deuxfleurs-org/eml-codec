use bounded_static::ToStatic;
#[cfg(feature = "tracing")]
use tracing::warn;

#[cfg(feature = "arbitrary")]
use crate::fuzz_eq::FuzzEq;
#[cfg(feature = "tracing-discard")]
use crate::utils::bytes_to_display_string;
use crate::header;
use crate::imf::address::{address_list, nullable_address_list, AddressList};
use crate::imf::datetime::{date_time, DateTime};
use crate::imf::identification::{msg_id, msg_list, MessageID, MessageIDList};
use crate::imf::mailbox::{mailbox, mailbox_list, MailboxList, MailboxRef};
use crate::imf::mime::{version, Version};
use crate::imf::trace::{received_log, return_path, ReceivedLog, ReturnPath};
use crate::text::misc_token::{phrase_list, unstructured, PhraseList, Unstructured};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
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
    #[cfg_attr(feature = "arbitrary", fuzz_eq(use_eq))]
    Comments(usize),
    #[cfg_attr(feature = "arbitrary", fuzz_eq(use_eq))]
    Keywords(usize),
    #[cfg_attr(feature = "arbitrary", fuzz_eq(use_eq))]
    Trace(usize),
    MIMEVersion,
}

#[derive(Clone, Debug, PartialEq, ToStatic)]
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

#[derive(Debug, Clone, Copy)]
pub enum InvalidField {
    Name,
    Body,
    NeedsDiscard,
}

impl<'a> TryFrom<&header::FieldRaw<'a>> for Field<'a> {
    type Error = InvalidField;

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", name = "imf::field::Field::try_from")
    )]
    fn try_from(f: &header::FieldRaw<'a>) -> Result<Self, Self::Error> {
        fn bind_res<T, U, F>(res: nom::IResult<&[u8], T>, f: F) -> Result<U, InvalidField>
            where F: Fn(T) -> Result<U, InvalidField>
        {
            match res {
                Ok((b"", content)) => f(content),
                Ok((_rest, _)) => {
                    // return an error if we haven't parsed the full value
                    #[cfg(feature = "tracing-discard")]
                    warn!(rest = bytes_to_display_string(_rest),
                          "leftover input after parsing");
                    Err(InvalidField::Body)
                },
                Err(_) => Err(InvalidField::Body)
            }
        }
        fn map_res<T, U, F>(res: nom::IResult<&[u8], T>, f: F) -> Result<U, InvalidField>
            where F: Fn(T) -> U
        {
            bind_res(res, |x| Ok(f(x)))
        }

        match f.name.bytes().to_ascii_lowercase().as_slice() {
            b"date" => map_res(date_time(f.body), Field::Date),
            b"from" => map_res(mailbox_list(f.body), Field::From),
            b"sender" => map_res(mailbox(f.body), Field::Sender),
            b"reply-to" => bind_res(nullable_address_list(f.body), |addrs| {
                if addrs.is_empty() {
                    Err(InvalidField::NeedsDiscard)
                } else {
                    Ok(Field::ReplyTo(addrs))
                }
            }),
            b"to" => map_res(address_list(f.body), Field::To),
            b"cc" => bind_res(nullable_address_list(f.body), |addrs| {
                if addrs.is_empty() {
                    Err(InvalidField::NeedsDiscard)
                } else {
                    Ok(Field::Cc(addrs))
                }
            }),
            b"bcc" => map_res(nullable_address_list(f.body), Field::Bcc),
            b"message-id" => map_res(msg_id(f.body), Field::MessageID),
            // TODO: obs-in-reply-to
            b"in-reply-to" => map_res(msg_list(f.body), Field::InReplyTo),
            // TODO: obs-references
            b"references" => map_res(msg_list(f.body), Field::References),
            b"subject" => map_res(unstructured(f.body), Field::Subject),
            b"comments" => map_res(unstructured(f.body), Field::Comments),
            b"keywords" => map_res(phrase_list(f.body), Field::Keywords),
            b"return-path" => map_res(return_path(f.body), Field::ReturnPath),
            b"received" => map_res(received_log(f.body), Field::Received),
            b"mime-version" => map_res(version(f.body), Field::MIMEVersion),
            _ => return Err(InvalidField::Name),
        }
    }
}

pub fn is_imf_header(name: &header::FieldName) -> bool {
    match name.bytes().to_ascii_lowercase().as_slice() {
        b"date" |
        b"from" |
        b"sender" |
        b"reply-to" |
        b"to" |
        b"cc" |
        b"bcc" |
        b"message-id" |
        b"in-reply-to" |
        b"references" |
        b"subject" |
        b"comments" |
        b"keywords" |
        b"return-path" |
        b"received" |
        b"mime-version" => true,
        _ => false,
    }
}
