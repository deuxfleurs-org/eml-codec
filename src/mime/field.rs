use bounded_static::ToStatic;
use nom::combinator::map;
#[cfg(feature = "tracing")]
use tracing::warn;

#[cfg(feature = "arbitrary")]
use crate::fuzz_eq::FuzzEq;
use crate::header;
use crate::imf::identification::{msg_id, MessageID};
use crate::mime::mechanism::{mechanism, Mechanism};
use crate::mime::r#type::{naive_type, AnyType, NaiveType};
use crate::print::{Formatter, Print};
use crate::text::misc_token::{unstructured, Unstructured};
#[cfg(feature = "tracing-unsupported")]
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
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub enum Field<'a> {
    Type(AnyType<'a>),
    TransferEncoding(Mechanism<'a>),
    ID(MessageID<'a>),
    Description(Unstructured<'a>),
}

impl<'a> Field<'a> {
    pub fn raw_name(&self) -> header::FieldName<'static> {
        match self {
            Field::Type(_) => header::FieldName(b"Content-Type".into()),
            Field::TransferEncoding(_) => header::FieldName(b"Content-Transfer-Encoding".into()),
            Field::ID(_) => header::FieldName(b"Content-Id".into()),
            Field::Description(_) => header::FieldName(b"Content-Description".into()),
        }
    }
}
impl<'a> Print for Field<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        match self {
            Self::Type(nt) => header::print(fmt, b"Content-Type", nt),
            Self::TransferEncoding(enc) => header::print(fmt, b"Content-Transfer-Encoding", enc),
            Self::ID(id) => header::print(fmt, b"Content-Id", id),
            Self::Description(desc) => {
                header::print_unstructured(fmt, b"Content-Description", desc)
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, ToStatic)]
pub enum NaiveField<'a> {
    Type(NaiveType<'a>),
    TransferEncoding(Mechanism<'a>),
    ID(MessageID<'a>),
    Description(Unstructured<'a>),
}

#[derive(Clone, Copy, Debug)]
pub enum InvalidField {
    Name,
    Body,
}

impl<'a> TryFrom<&header::FieldRaw<'a>> for NaiveField<'a> {
    type Error = InvalidField;

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "mime::field::Field::try_from")
    )]
    fn try_from(f: &header::FieldRaw<'a>) -> Result<Self, Self::Error> {
        let content = match f.name.bytes().to_ascii_lowercase().as_slice() {
            b"content-type" => map(naive_type, NaiveField::Type)(f.body),
            b"content-transfer-encoding" => map(mechanism, NaiveField::TransferEncoding)(f.body),
            b"content-id" => map(msg_id, NaiveField::ID)(f.body),
            b"content-description" => map(unstructured, NaiveField::Description)(f.body),
            _ => return Err(InvalidField::Name),
        };

        match content {
            Ok((b"", content)) => Ok(content),
            Ok((_rest, _)) => {
                // return an error if we haven't parsed the full value
                #[cfg(feature = "tracing-unsupported")]
                warn!(rest = %bytes_to_trace_string(_rest),
                      "leftover input after parsing");
                Err(InvalidField::Body)
            }
            Err(_) => Err(InvalidField::Body),
        }
    }
}

pub fn is_mime_header(name: &header::FieldName) -> bool {
    match name.bytes().to_ascii_lowercase().as_slice() {
        b"content-type" | b"content-transfer-encoding" | b"content-id" | b"content-description" => {
            true
        }
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
            (input, hdrs.iter().flat_map(NaiveField::try_from).collect()),
            (
                &b"This is a multipart message.\n\n"[..],
                vec![
                    NaiveField::Type(NaiveType {
                        main: MIMEAtom(b"multipart"[..].into()),
                        sub: MIMEAtom(b"alternative"[..].into()),
                        params: vec![Parameter {
                            name: MIMEAtom(b"boundary"[..].into()),
                            value: MIMEWord::Quoted(QuotedString(vec![
                                "b1_e376dc71bafc953c0b0fdeb9983a9956"[..].into()
                            ])),
                        }]
                    }),
                    NaiveField::TransferEncoding(Mechanism::_7Bit),
                ],
            ),
        );
    }
}
