use bounded_static::ToStatic;
use nom::combinator::map;

use crate::display_bytes::{Print, Formatter};
use crate::header;
use crate::imf::address::{address_list, mailbox_list, nullable_address_list, AddressList};
use crate::imf::datetime::{date_time, DateTime};
use crate::imf::identification::{msg_id, msg_list, MessageID, MessageIDList};
use crate::imf::mailbox::{mailbox, MailboxList, MailboxRef};
use crate::imf::mime::{version, Version};
use crate::imf::trace::{received_log, return_path, ReceivedLog, ReturnPath};
use crate::text::misc_token::{phrase_list, unstructured, PhraseList, Unstructured};

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
    Keywords(PhraseList<'a>),

    // 3.6.6   Resent Fields (not implemented)
    // 3.6.7   Trace Fields
    Received(ReceivedLog<'a>),
    ReturnPath(ReturnPath<'a>),

    MIMEVersion(Version),
}
impl<'a> TryFrom<&header::FieldRaw<'a>> for Field<'a> {
    type Error = ();
    fn try_from(f: &header::FieldRaw<'a>) -> Result<Self, Self::Error> {
        let content = match f {
            header::FieldRaw::Good(key, value) => {
                match key.bytes().to_ascii_lowercase().as_slice() {
                    b"date" => map(date_time, Field::Date)(value),
                    b"from" => map(mailbox_list, Field::From)(value),
                    b"sender" => map(mailbox, Field::Sender)(value),
                    b"reply-to" => map(address_list, Field::ReplyTo)(value),
                    b"to" => map(address_list, Field::To)(value),
                    b"cc" => map(address_list, Field::Cc)(value),
                    b"bcc" => map(nullable_address_list, Field::Bcc)(value),
                    b"message-id" => map(msg_id, Field::MessageID)(value),
                    b"in-reply-to" => map(msg_list, Field::InReplyTo)(value),
                    b"references" => map(msg_list, Field::References)(value),
                    b"subject" => map(unstructured, Field::Subject)(value),
                    b"comments" => map(unstructured, Field::Comments)(value),
                    b"keywords" => map(phrase_list, Field::Keywords)(value),
                    b"return-path" => map(return_path, Field::ReturnPath)(value),
                    b"received" => map(received_log, Field::Received)(value),
                    b"mime-version" => map(version, Field::MIMEVersion)(value),
                    _ => return Err(()),
                }
            }
            _ => return Err(()),
        };

        content.map(|(_, content)| content).or(Err(()))
    }
}

impl<'a> Print for Field<'a> {
    fn print(&self, fmt: &mut impl Formatter) -> std::io::Result<()> {
        match self {
            Field::Date(datetime) => p(fmt, b"Date", datetime),

            Field::From(mboxlist) => p(fmt, b"From", mboxlist),
            Field::Sender(mbox) => p(fmt, b"Sender", mbox),
            Field::ReplyTo(addrlist) => p(fmt, b"Reply-To", addrlist),

            Field::To(addrlist) => p(fmt, b"To", addrlist),
            Field::Cc(addrlist) => p(fmt, b"Cc", addrlist),
            Field::Bcc(addrlist) => p(fmt, b"Bcc", addrlist),

            Field::MessageID(id) => p(fmt, b"Message-ID", id),
            Field::InReplyTo(refs) => p(fmt, b"In-Reply-To", refs),
            Field::References(refs) => p(fmt, b"References", refs),

            Field::Subject(unstr) => p_unstructured(fmt, b"Subject", unstr),
            Field::Comments(unstr) => p_unstructured(fmt, b"Comments", unstr),
            Field::Keywords(kwds) => p(fmt, b"Keywords", kwds),

            Field::Received(log) => p(fmt, b"Received", log),
            Field::ReturnPath(path) => p(fmt, b"Return-Path", path),

            Field::MIMEVersion(ver) => p(fmt, b"MIME-Version", ver),
        }
    }
}

fn p<T: Print>(fmt: &mut impl Formatter, name: &[u8], body: &T) ->
    std::io::Result<()>
{
    fmt.write_bytes(name)?;
    fmt.write_bytes(b":")?;
    fmt.write_fws()?;
    body.print(fmt)?;
    fmt.write_crlf()
}

fn p_unstructured(fmt: &mut impl Formatter, name: &[u8], body: &Unstructured<'_>) ->
    std::io::Result<()>
{
    fmt.write_bytes(name)?;
    fmt.write_bytes(b":")?;
    // all text is significant in an unstructured field; do not add FWS
    body.print(fmt)?;
    fmt.write_crlf()
}
