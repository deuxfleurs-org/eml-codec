use std::convert::From;

use nom::{
    IResult,
    bytes::complete::{take_while1, tag},
    character::complete::space0,
    sequence::{terminated, tuple},
};

#[derive(Debug, PartialEq)]
pub enum Field<'a> {
    // 3.6.1.  The Origination Date Field
    Date(&'a str),

    // 3.6.2.  Originator Fields
    From(&'a str),
    Sender(&'a str),
    ReplyTo(&'a str),

    // 3.6.3.  Destination Address Fields
    To(&'a str),
    Cc(&'a str),
    Bcc(&'a str),

    // 3.6.4.  Identification Fields
    MessageID(&'a str),
    InReplyTo(&'a str),
    References(&'a str),

    // 3.6.5.  Informational Fields
    Subject(&'a str),
    Comments(&'a str),
    Keywords(&'a str),

    // 3.6.6   Resent Fields (not implemented)
    // 3.6.7   Trace Fields
    Received(&'a str),
    ReturnPath(&'a str),

    // 3.6.8.  Optional Fields
    Optional(&'a str, &'a str),

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
        tuple((space0, tag(":"), space0))
    )(input)
}

fn correct_field(input: &str) -> IResult<&str, Field> {
    field_name(input)
        .map(|(rest, name)| ("", match name.to_lowercase().as_ref() {
            "date" => Date(rest),

            "from" => From(rest),
            "sender" => Sender(rest),
            "reply-to" => ReplyTo(rest),

            "to" => To(rest),
            "cc" => Cc(rest),
            "bcc" => Bcc(rest),

            "message-id" => MessageID(rest),
            "in-reply-to" => InReplyTo(rest),
            "references" => References(rest),

            "subject" => Subject(rest),
            "comments" => Comments(rest),
            "keywords" => Keywords(rest),

            "return-path" => ReturnPath(rest),
            "received" => Received(rest),

            _ => Optional(name, rest),
    }))
}
