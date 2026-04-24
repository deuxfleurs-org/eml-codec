use bounded_static::ToStatic;
#[cfg(feature = "tracing")]
use tracing::warn;

#[cfg(feature = "arbitrary")]
use crate::fuzz_eq::FuzzEq;
#[cfg(feature = "tracing-unsupported")]
use crate::utils::bytes_to_trace_string;
use crate::header;
use crate::imf::address::{nullable_address_list, AddressList};
use crate::imf::datetime::{date_time, DateTime};
use crate::imf::identification::{msg_id, nullable_msg_list, MessageID, MessageIDList};
use crate::imf::mailbox::{mailbox, mailbox_list, MailboxList, MailboxRef};
use crate::imf::mime::{version, Version};
use crate::imf::trace::{return_path, ReturnPath};
use crate::print::{Print, Formatter};
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
    MessageID,
    InReplyTo,
    References,
    Subject,
    #[cfg_attr(feature = "arbitrary", fuzz_eq(use_eq))]
    Comments(usize),
    #[cfg_attr(feature = "arbitrary", fuzz_eq(use_eq))]
    Keywords(usize),
    #[cfg_attr(feature = "arbitrary", fuzz_eq(use_eq))]
    Trace(usize), // either a Received or ReturnPath field
    MIMEVersion,
}

#[derive(Clone, Debug, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
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
    Keywords(PhraseList<'a>),

    // 3.6.6   Resent Fields (not implemented)
    // 3.6.7   Trace Fields
    Received(Unstructured<'a>),
    ReturnPath(ReturnPath<'a>),

    // MIME
    MIMEVersion(Version),
}

impl<'a> Print for Field<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        match self {
            Self::Date(d) => header::print(fmt, b"Date", d),
            Self::From(mboxl) => header::print(fmt, b"From", mboxl),
            Self::Sender(mbox) => header::print(fmt, b"Sender", mbox),
            Self::ReplyTo(addrs) => header::print(fmt, b"Reply-To", addrs),
            Self::To(addrs) => header::print(fmt, b"To", addrs),
            Self::Cc(addrs) => header::print(fmt, b"Cc", addrs),
            Self::Bcc(addrs) => header::print(fmt, b"Bcc", addrs),
            Self::MessageID(id) => header::print(fmt, b"Message-ID", id),
            Self::InReplyTo(ids) => header::print(fmt, b"In-Reply-To", ids),
            Self::References(ids) => header::print(fmt, b"References", ids),
            Self::Subject(u) => header::print_unstructured(fmt, b"Subject", u),
            Self::Comments(u) => header::print_unstructured(fmt, b"Comments", u),
            Self::Keywords(l) => header::print(fmt, b"Keywords", l),
            Self::Received(u) => header::print_unstructured(fmt, b"Received", u),
            Self::ReturnPath(p) => header::print(fmt, b"Return-Path", p),
            Self::MIMEVersion(v) => header::print(fmt, b"MIME-Version", v),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum InvalidField {
    /// The field name is not a known IMF field
    Name,
    /// The field body could not be parsed
    Body,
    /// The field could be parsed but represents a dummy value that is not part
    /// of the RFC-strict syntax. It must be discarded (no meaningful data is
    /// lost).
    NeedsDiscard,
}

impl<'a> TryFrom<&header::FieldRaw<'a>> for Field<'a> {
    type Error = InvalidField;

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "imf::field::Field::try_from")
    )]
    fn try_from(f: &header::FieldRaw<'a>) -> Result<Self, Self::Error> {
        fn bind_res<T, U, F>(res: nom::IResult<&[u8], T>, f: F) -> Result<U, InvalidField>
        where F: Fn(T) -> Result<U, InvalidField>
        {
            match res {
                Ok((b"", content)) => f(content),
                Ok((_rest, _)) => {
                    // return an error if we haven't parsed the full value
                    #[cfg(feature = "tracing-unsupported")]
                    warn!(rest = %bytes_to_trace_string(_rest),
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
            b"to" => bind_res(nullable_address_list(f.body), |addrs| {
                if addrs.is_empty() {
                    Err(InvalidField::NeedsDiscard)
                } else {
                    Ok(Field::To(addrs))
                }
            }),
            b"cc" => bind_res(nullable_address_list(f.body), |addrs| {
                if addrs.is_empty() {
                    Err(InvalidField::NeedsDiscard)
                } else {
                    Ok(Field::Cc(addrs))
                }
            }),
            b"bcc" => map_res(nullable_address_list(f.body), Field::Bcc),
            b"message-id" => map_res(msg_id(f.body), Field::MessageID),
            b"in-reply-to" => bind_res(nullable_msg_list(f.body), |msgl| {
                // the obs syntax allows empty message lists, but not the normal
                // syntax. we drop them.
                if msgl.is_empty() {
                    Err(InvalidField::NeedsDiscard)
                } else {
                    Ok(Field::InReplyTo(msgl))
                }
            }),
            b"references" => bind_res(nullable_msg_list(f.body), |msgl| {
                // the obs syntax allows empty message lists, but not the normal
                // syntax. we drop them.
                if msgl.is_empty() {
                    Err(InvalidField::NeedsDiscard)
                } else {
                    Ok(Field::References(msgl))
                }
            }),
            b"subject" => map_res(unstructured(f.body), Field::Subject),
            b"comments" => map_res(unstructured(f.body), Field::Comments),
            b"keywords" => bind_res(phrase_list(f.body), |opt| {
                // the obs syntax allows empty phrase lists, but not the normal
                // syntax. we drop them.
                match opt {
                    None => Err(InvalidField::NeedsDiscard),
                    Some(kwds) => Ok(Field::Keywords(kwds)),
                }
            }),
            b"return-path" => map_res(return_path(f.body), Field::ReturnPath),
            b"received" => map_res(unstructured(f.body), Field::Received),
            b"mime-version" => map_res(version(f.body), Field::MIMEVersion),
            _ => return Err(InvalidField::Name),
        }
    }
}

impl<'a> TryFrom<&header::Unstructured<'a>> for Field<'static> {
    type Error = InvalidField;

    fn try_from(u: &header::Unstructured<'a>) -> Result<Self, Self::Error> {
        use bounded_static::IntoBoundedStatic;
        use std::borrow::Cow;
        let bytes_body: Cow<[u8]> = match u.raw_body.0 {
            Some(s) => s.into(),
            None => u.body.to_string_keep_obs().into_bytes().into(),
        };
        let hdr = header::FieldRaw { name: u.name.clone(), body: &bytes_body };
        Field::try_from(&hdr).map(IntoBoundedStatic::into_static)
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
