use bounded_static::ToStatic;
use nom::combinator::map;

use crate::header;
use crate::imf::address::{address_list, mailbox_list, nullable_address_list, AddressList};
use crate::imf::datetime::{date_time, DateTime};
use crate::imf::identification::{msg_id, msg_list, MessageID, MessageIDList};
use crate::imf::mailbox::{mailbox, AddrSpec, MailboxList, MailboxRef};
use crate::imf::mime::{version, Version};
use crate::imf::trace::{received_log, return_path, ReceivedLog};
use crate::text::misc_token::{phrase_list, unstructured, PhraseList, Unstructured};

#[derive(Debug, PartialEq, ToStatic)]
pub enum Field<'a> {
    // 3.6.1.  The Origination Date Field
    Date(Option<DateTime>),

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
