use nom::{
    IResult,
};

use crate::rfc5322::address::{MailboxList, AddressList};
use crate::rfc5322::mailbox::MailboxRef;
use crate::rfc5322::identification::{MessageId, MessageIdList};
use crate::rfc5322::trace::ReceivedLog;
use crate::text::misc_token::{Unstructured, PhraseList};

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
    ReturnPath(Option<AddrSpec<'a>>),

    MIMEVersion(Version<'a>),
    Optional(&'a [u8], Unstructured<'a>),
}

pub fn field(input: &[u8]) -> IResult<&[u8], Field<'a>> {
    let (name, rest) = field_name(input)?;
    match name.to_lowercase().as_ref() {
        "date" => datetime::section(rest).map(Field::Date),
        "from" => mailbox_list(rest).map(Field::From),
        "sender" => mailbox(rest).map(Field::Sender),
        "reply-to" => address_list(rest).map(Field::ReplyTo),

        "to" => address_list(rest).map(Field::To),
        "cc" => address_list(rest).map(Field::Cc),
        "bcc" => nullable_address_list(rest).map(Field::Bcc), 

        "message-id" => msg_id(rest).map(Field::MessageID),
        "in-reply-to" => msg_list(rest).map(Field::InReplyTo),
        "references" => msg_list(rest).map(Field::References),

        "subject" => unstructured(rest).map(Field::Subject),
        "comments" => unstructured(rest).map(Field::Comments),
        "keywords" => phrase_list(rest).map(Field::Keywords),

        "return-path" => return_path(rest).map(Field::ReturnPath), 
        "received" => received_log(rest).map(Field::ReceivedLog), 

        "mime-version" => version(rest).map(Field::MIMEVersion), 
         _ => unstructured(rest).map(|v| Field::Optional(name, v)),
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
fn field_name(input: &[u8]) -> IResult<&[u8], &[u8]> {
    terminated(
        take_while1(|c| c >= 0x21 && c <= 0x7E && c != 0x3A),
        tuple((space0, tag(b":"), space0)),
    )(input)
}
