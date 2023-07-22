use chrono::{DateTime, FixedOffset};
use nom::{
    IResult,
    branch::alt,
    combinator::map,
    multi::many0,
    sequence::{preceded, terminated},
};

use crate::text::whitespace::{obs_crlf, foldable_line};
use crate::rfc5322::address::{AddressList, address_list, nullable_address_list, mailbox_list};
use crate::rfc5322::datetime::section as date;
use crate::rfc5322::mailbox::{MailboxRef, MailboxList, AddrSpec, mailbox};
use crate::rfc5322::identification::{MessageID, MessageIDList, msg_id, msg_list};
use crate::rfc5322::trace::{ReceivedLog, return_path, received_log};
use crate::rfc5322::mime::{Version, version};
use crate::rfc5322::message::Message;
use crate::header::*;
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


#[derive(Debug, PartialEq)]
pub struct FieldList<'a>(pub Vec<Field<'a>>);
impl<'a> FieldList<'a> {
    pub fn message(self) -> Message<'a> {
        Message::from_iter(self.0)
    }
}

pub fn field(input: &[u8]) -> IResult<&[u8], Field> {
    terminated(alt((
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
    )), obs_crlf)(input)
}

pub fn header(input: &[u8]) -> IResult<&[u8], CompFieldList<Field>> {
    map(terminated(many0(alt((
        map(field, CompField::Known),
        map(opt_field, |(k,v)| CompField::Unknown(k,v)),
        map(foldable_line, CompField::Bad),
    ))), obs_crlf), CompFieldList)(input)
}


#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, TimeZone};
    use crate::rfc5322::mailbox::*;
    use crate::rfc5322::address::*;
    use crate::text::misc_token::*;

    #[test]
    fn test_header() {
        let fullmail = b"Date: 7 Mar 2023 08:00:00 +0200
From: someone@example.com
To: someone_else@example.com
Subject: An RFC 822 formatted message

This is the plain text body of the message. Note the blank line
between the header information and the body of the message.";

        assert_eq!(
            map(header, |v| FieldList(v.known()).message())(fullmail),
            Ok((
                &b"This is the plain text body of the message. Note the blank line\nbetween the header information and the body of the message."[..],
                Message {
                    date: Some(FixedOffset::east_opt(2 * 3600).unwrap().with_ymd_and_hms(2023, 3, 7, 8, 0, 0).unwrap()),
                    from: vec![MailboxRef {
                        name: None,
                        addrspec: AddrSpec {
                            local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(&b"someone"[..]))]),
                            domain: Domain::Atoms(vec![&b"example"[..], &b"com"[..]]),
                        }
                    }],
                    to: vec![AddressRef::Single(MailboxRef {
                        name: None,
                        addrspec: AddrSpec {
                            local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(&b"someone_else"[..]))]),
                            domain: Domain::Atoms(vec![&b"example"[..], &b"com"[..]]),
                        }
                    })],
                    subject: Some(Unstructured(vec![
                        UnstrToken::Plain(&b"An"[..]),
                        UnstrToken::Plain(&b"RFC"[..]),
                        UnstrToken::Plain(&b"822"[..]), 
                        UnstrToken::Plain(&b"formatted"[..]), 
                        UnstrToken::Plain(&b"message"[..]),
                    ])),
                    ..Message::default()
                }
            )),
        )
    }
}
