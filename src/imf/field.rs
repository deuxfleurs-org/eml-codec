use chrono::{DateTime, FixedOffset};
use nom::{
    branch::alt,
    combinator::map,
    sequence::{preceded, terminated},
    IResult,
};

use crate::header::{field_name};
use crate::imf::address::{address_list, mailbox_list, nullable_address_list, AddressList};
use crate::imf::datetime::section as date;
use crate::imf::identification::{msg_id, msg_list, MessageID, MessageIDList};
use crate::imf::mailbox::{mailbox, AddrSpec, MailboxList, MailboxRef};
use crate::imf::mime::{version, Version};
use crate::imf::trace::{received_log, return_path, ReceivedLog};
use crate::text::misc_token::{phrase_list, unstructured, PhraseList, Unstructured};
use crate::text::whitespace::obs_crlf;

#[derive(Debug, PartialEq)]
pub enum Field<'a> {
    // 3.6.1.  The Origination Date Field
    Date(Option<DateTime<FixedOffset>>),

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
    Received(ReceivedLog<'a>),
    ReturnPath(Option<AddrSpec<'a>>),

    MIMEVersion(Version),
}
/*impl<'a> From<header::Field<'a>> for Field<'a> {
    fn from(raw: header::Field
}*/

pub fn field(input: &[u8]) -> IResult<&[u8], Field> {
    terminated(
        alt((
            preceded(field_name(b"date"), map(date, Field::Date)),
            preceded(field_name(b"from"), map(mailbox_list, Field::From)),
            preceded(field_name(b"sender"), map(mailbox, Field::Sender)),
            preceded(field_name(b"reply-to"), map(address_list, Field::ReplyTo)),
            preceded(field_name(b"to"), map(address_list, Field::To)),
            preceded(field_name(b"cc"), map(address_list, Field::Cc)),
            preceded(field_name(b"bcc"), map(nullable_address_list, Field::Bcc)),
            preceded(field_name(b"message-id"), map(msg_id, Field::MessageID)),
            preceded(field_name(b"in-reply-to"), map(msg_list, Field::InReplyTo)),
            preceded(field_name(b"references"), map(msg_list, Field::References)),
            preceded(field_name(b"subject"), map(unstructured, Field::Subject)),
            preceded(field_name(b"comments"), map(unstructured, Field::Comments)),
            preceded(field_name(b"keywords"), map(phrase_list, Field::Keywords)),
            preceded(
                field_name(b"return-path"),
                map(return_path, Field::ReturnPath),
            ),
            preceded(field_name(b"received"), map(received_log, Field::Received)),
            preceded(
                field_name(b"mime-version"),
                map(version, Field::MIMEVersion),
            ),
        )),
        obs_crlf,
    )(input)
}
