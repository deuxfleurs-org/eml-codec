use chrono::{DateTime, FixedOffset};
use nom::{
    IResult,
    branch::alt,
    bytes::complete::{tag, tag_no_case, take_while1},
    character::complete::space0,
    combinator::map,
    sequence::{pair, preceded, terminated, tuple},
};

use crate::rfc5322::address::{AddressList, address_list, nullable_address_list, mailbox_list};
use crate::rfc5322::datetime::section as date;
use crate::rfc5322::mailbox::{MailboxRef, MailboxList, AddrSpec, mailbox};
use crate::rfc5322::identification::{MessageID, MessageIDList, msg_id, msg_list};
use crate::rfc5322::trace::{ReceivedLog, return_path, received_log};
use crate::rfc5322::mime::{Version, version};
use crate::text::misc_token::{Unstructured, PhraseList, unstructured, phrase_list};

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

pub fn field(input: &[u8]) -> IResult<&[u8], Field> {
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

        preceded(field_name(b"return-path"), map(return_path, Field::ReturnPath)), 
        preceded(field_name(b"received"), map(received_log, Field::Received)), 

        preceded(field_name(b"mime-version"), map(version, Field::MIMEVersion)), 
    ))(input)
}


fn field_name<'a>(name: &'static [u8]) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], &'a [u8]> {
    move |input| {
        terminated(
            tag_no_case(name),
            tuple((space0, tag(b":"), space0)),
        )(input)
    }
}

/// Optional field
///
/// ```abnf
/// field      =   field-name ":" unstructured CRLF
/// field-name =   1*ftext
/// ftext      =   %d33-57 /          ; Printable US-ASCII
///                %d59-126           ;  characters not including
///                                   ;  ":".
/// ```
fn opt_field(input: &[u8]) -> IResult<&[u8], (&[u8], Unstructured)> {
    pair(
        terminated(
            take_while1(|c| c >= 0x21 && c <= 0x7E && c != 0x3A),
            tuple((space0, tag(b":"), space0)),
        ),
        unstructured,
    )(input)
} 

// @TODO write a parse header function
