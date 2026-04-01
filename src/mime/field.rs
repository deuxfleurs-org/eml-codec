use bounded_static::ToStatic;
use nom::combinator::map;
#[cfg(feature = "tracing")]
use tracing::warn;

#[cfg(feature = "arbitrary")]
use crate::fuzz_eq::FuzzEq;
use crate::header;
use crate::imf::identification::{msg_id, MessageID};
use crate::mime::mechanism::{mechanism, Mechanism};
use crate::mime::r#type::{naive_type, NaiveType};
use crate::text::misc_token::{unstructured, Unstructured};
#[cfg(feature = "tracing-discard")]
use crate::utils::bytes_to_trace_string;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub enum Entry {
    Type,
    TransferEncoding,
    ID,
    Description,
}

#[derive(Clone, Debug, PartialEq, ToStatic)]
pub enum Content<'a> {
    Type(NaiveType<'a>),
    TransferEncoding(Mechanism<'a>),
    ID(MessageID<'a>),
    Description(Unstructured<'a>),
}
#[allow(dead_code)]
impl<'a> Content<'a> {
    pub fn ctype(&'a self) -> Option<&'a NaiveType<'a>> {
        match self {
            Content::Type(v) => Some(v),
            _ => None,
        }
    }
    pub fn transfer_encoding(&'a self) -> Option<&'a Mechanism<'a>> {
        match self {
            Content::TransferEncoding(v) => Some(v),
            _ => None,
        }
    }
    pub fn id(&'a self) -> Option<&'a MessageID<'a>> {
        match self {
            Content::ID(v) => Some(v),
            _ => None,
        }
    }
    pub fn description(&'a self) -> Option<&'a Unstructured<'a>> {
        match self {
            Content::Description(v) => Some(v),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum InvalidField {
    Name,
    Body,
}

impl<'a> TryFrom<&header::FieldRaw<'a>> for Content<'a> {
    type Error = InvalidField;

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", name = "mime::field::Content::try_from")
    )]
    fn try_from(f: &header::FieldRaw<'a>) -> Result<Self, Self::Error> {
        let content = match f.name.bytes().to_ascii_lowercase().as_slice() {
            b"content-type" => map(naive_type, Content::Type)(f.body),
            b"content-transfer-encoding" => map(mechanism, Content::TransferEncoding)(f.body),
            b"content-id" => map(msg_id, Content::ID)(f.body),
            b"content-description" => map(unstructured, Content::Description)(f.body),
            _ => return Err(InvalidField::Name),
        };

        match content {
            Ok((b"", content)) => Ok(content),
            Ok((_rest, _)) => {
                // return an error if we haven't parsed the full value
                #[cfg(feature = "tracing-discard")]
                warn!(rest = %bytes_to_trace_string(_rest),
                      "leftover input after parsing");
                Err(InvalidField::Body)
            },
            Err(_) => Err(InvalidField::Body),
        }
    }
}

pub fn is_mime_header(name: &header::FieldName) -> bool {
    match name.bytes().to_ascii_lowercase().as_slice() {
        b"content-type" |
        b"content-transfer-encoding" |
        b"content-id" |
        b"content-description" => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header;
    use crate::mime::r#type::*;
    use crate::text::misc_token::MIMEWord;
    use crate::text::quoted::QuotedString;
    use crate::text::words::MIMEAtom;

    #[test]
    fn test_header() {
        let fullmail: &[u8] = r#"Date: Sat, 8 Jul 2023 07:14:29 +0200
From: Grrrnd Zero <grrrndzero@example.org>
To: John Doe <jdoe@machine.example>
Subject: Re: Saying Hello
Message-ID: <NTAxNzA2AC47634Y366BAMTY4ODc5MzQyODY0ODY5@www.grrrndzero.org>
MIME-Version: 1.0
Content-Type: multipart/alternative;
 boundary="b1_e376dc71bafc953c0b0fdeb9983a9956"
Content-Transfer-Encoding: 7bit

This is a multipart message.

"#
        .as_bytes();

        let (input, hdrs) = header::header_kv(fullmail);

        assert_eq!(
            (input, hdrs.iter().flat_map(Content::try_from).collect()),
            (
                &b"This is a multipart message.\n\n"[..],
                vec![
                    Content::Type(NaiveType {
                        main: MIMEAtom(b"multipart"[..].into()),
                        sub: MIMEAtom(b"alternative"[..].into()),
                        params: vec![Parameter {
                            name: MIMEAtom(b"boundary"[..].into()),
                            value: MIMEWord::Quoted(QuotedString(vec![
                                "b1_e376dc71bafc953c0b0fdeb9983a9956"[..].into()
                            ])),
                        }]
                    }),
                    Content::TransferEncoding(Mechanism::_7Bit),
                ],
            ),
        );
    }
}
