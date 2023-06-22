use std::convert::From;

use nom::{
    bytes::complete::{tag, take_while1},
    character::complete::space0,
    sequence::{terminated, tuple},
    IResult,
};

#[derive(Debug, PartialEq)]
pub struct DateTime<'a>(pub &'a str);

#[derive(Debug, PartialEq)]
pub struct MailboxList<'a>(pub &'a str);

#[derive(Debug, PartialEq)]
pub struct Mailbox<'a>(pub &'a str);

#[derive(Debug, PartialEq)]
pub struct AddressList<'a>(pub &'a str);

#[derive(Debug, PartialEq)]
pub struct NullableAddressList<'a>(pub &'a str);

#[derive(Debug, PartialEq)]
pub struct Identifier<'a>(pub &'a str);

#[derive(Debug, PartialEq)]
pub struct IdentifierList<'a>(pub &'a str);

#[derive(Debug, PartialEq)]
pub struct Unstructured<'a>(pub &'a str);

#[derive(Debug, PartialEq)]
pub struct PhraseList<'a>(pub &'a str);

#[derive(Debug, PartialEq)]
pub struct ReceivedLog<'a>(pub &'a str);

#[derive(Debug, PartialEq)]
pub struct Path<'a>(pub &'a str);

#[derive(Debug, PartialEq)]
pub enum Field<'a> {
    // 3.6.1.  The Origination Date Field
    Date(DateTime<'a>),

    // 3.6.2.  Originator Fields
    From(MailboxList<'a>),
    Sender(Mailbox<'a>),
    ReplyTo(AddressList<'a>),

    // 3.6.3.  Destination Address Fields
    To(AddressList<'a>),
    Cc(AddressList<'a>),
    Bcc(NullableAddressList<'a>),

    // 3.6.4.  Identification Fields
    MessageID(Identifier<'a>),
    InReplyTo(IdentifierList<'a>),
    References(IdentifierList<'a>),

    // 3.6.5.  Informational Fields
    Subject(Unstructured<'a>),
    Comments(Unstructured<'a>),
    Keywords(PhraseList<'a>),

    // 3.6.6   Resent Fields (not implemented)
    // 3.6.7   Trace Fields
    Received(ReceivedLog<'a>),
    ReturnPath(Mailbox<'a>),

    // 3.6.8.  Optional Fields
    Optional(&'a str, Unstructured<'a>),

    // None
    Rescue(&'a str),
}
use Field::*;

impl<'a> From<&'a str> for Field<'a> {
    fn from(input: &'a str) -> Self {
        match correct_field(input) {
            Ok((_, field)) => field,
            Err(_) => Rescue(input),
        }
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
fn field_name(input: &str) -> IResult<&str, &str> {
    terminated(
        take_while1(|c| c >= '\x21' && c <= '\x7E' && c != '\x3A'),
        tuple((space0, tag(":"), space0)),
    )(input)
}

fn correct_field(input: &str) -> IResult<&str, Field> {
    field_name(input).map(|(rest, name)| {
        (
            "",
            match name.to_lowercase().as_ref() {
                "date" => Date(DateTime(rest)),

                "from" => From(MailboxList(rest)),
                "sender" => Sender(Mailbox(rest)),
                "reply-to" => ReplyTo(AddressList(rest)),

                "to" => To(AddressList(rest)),
                "cc" => Cc(AddressList(rest)),
                "bcc" => Bcc(NullableAddressList(rest)),

                "message-id" => MessageID(Identifier(rest)),
                "in-reply-to" => InReplyTo(IdentifierList(rest)),
                "references" => References(IdentifierList(rest)),

                "subject" => Subject(Unstructured(rest)),
                "comments" => Comments(Unstructured(rest)),
                "keywords" => Keywords(PhraseList(rest)),

                "return-path" => ReturnPath(Mailbox(rest)),
                "received" => Received(ReceivedLog(rest)),

                _ => Optional(name, Unstructured(rest)),
            },
        )
    })
}
