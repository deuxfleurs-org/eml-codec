use nom::{
    bytes::complete::tag,
    combinator::{map, opt},
    multi::many0,
    sequence::{preceded, terminated, tuple},
    IResult,
};
use std::fmt;

use crate::mime::charset::EmailCharset;
use crate::mime::{AnyMIME, NaiveMIME, MIME};
use crate::text::misc_token::{mime_word, MIMEWord};
use crate::text::words::mime_atom;

// --------- NAIVE TYPE
#[derive(PartialEq, Clone)]
pub struct NaiveType<'a> {
    pub main: &'a [u8],
    pub sub: &'a [u8],
    pub params: Vec<Parameter<'a>>,
}
impl<'a> fmt::Debug for NaiveType<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("mime::NaiveType")
            .field("main", &String::from_utf8_lossy(self.main))
            .field("sub", &String::from_utf8_lossy(self.sub))
            .field("params", &self.params)
            .finish()
    }
}
impl<'a> NaiveType<'a> {
    pub fn to_type(&self) -> AnyType {
        self.into()
    }
}
pub fn naive_type(input: &[u8]) -> IResult<&[u8], NaiveType> {
    map(
        tuple((mime_atom, tag("/"), mime_atom, parameter_list)),
        |(main, _, sub, params)| NaiveType { main, sub, params },
    )(input)
}

#[derive(PartialEq, Clone)]
pub struct Parameter<'a> {
    pub name: &'a [u8],
    pub value: MIMEWord<'a>,
}
impl<'a> fmt::Debug for Parameter<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("mime::Parameter")
            .field("name", &String::from_utf8_lossy(self.name))
            .field("value", &self.value)
            .finish()
    }
}

pub fn parameter(input: &[u8]) -> IResult<&[u8], Parameter> {
    map(
        tuple((mime_atom, tag(b"="), mime_word)),
        |(name, _, value)| Parameter { name, value },
    )(input)
}
pub fn parameter_list(input: &[u8]) -> IResult<&[u8], Vec<Parameter>> {
    terminated(many0(preceded(tag(";"), parameter)), opt(tag(";")))(input)
}

// MIME TYPES TRANSLATED TO RUST TYPING SYSTEM

#[derive(Debug, PartialEq)]
pub enum AnyType {
    // Composite types
    Multipart(Multipart),
    Message(Deductible<Message>),

    // Discrete types
    Text(Deductible<Text>),
    Binary(Binary),
}

impl<'a> From<&'a NaiveType<'a>> for AnyType {
    fn from(nt: &'a NaiveType<'a>) -> Self {
        match nt.main.to_ascii_lowercase().as_slice() {
            b"multipart" => Multipart::try_from(nt)
                .map(Self::Multipart)
                .unwrap_or(Self::Text(DeductibleText::default())),
            b"message" => Self::Message(DeductibleMessage::Explicit(Message::from(nt))),
            b"text" => Self::Text(DeductibleText::Explicit(Text::from(nt))),
            _ => Self::Binary(Binary::default()),
        }
    }
}

impl<'a> AnyType {
    pub fn to_mime(self, fields: NaiveMIME<'a>) -> AnyMIME<'a> {
        match self {
            Self::Multipart(interpreted_type) => AnyMIME::Mult(MIME::<Multipart> {
                interpreted_type,
                fields,
            }),
            Self::Message(interpreted_type) => AnyMIME::Msg(MIME::<DeductibleMessage> {
                interpreted_type,
                fields,
            }),
            Self::Text(interpreted_type) => AnyMIME::Txt(MIME::<DeductibleText> {
                interpreted_type,
                fields,
            }),
            Self::Binary(interpreted_type) => AnyMIME::Bin(MIME::<Binary> {
                interpreted_type,
                fields,
            }),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum Deductible<T: Default> {
    Inferred(T),
    Explicit(T),
}
impl<T: Default> Default for Deductible<T> {
    fn default() -> Self {
        Self::Inferred(T::default())
    }
}

// REAL PARTS

#[derive(Debug, PartialEq, Clone)]
pub struct Multipart {
    pub subtype: MultipartSubtype,
    pub boundary: String,
}
impl Multipart {
    pub fn main_type(&self) -> String {
        "multipart".into()
    }
}
impl<'a> TryFrom<&'a NaiveType<'a>> for Multipart {
    type Error = ();

    fn try_from(nt: &'a NaiveType<'a>) -> Result<Self, Self::Error> {
        nt.params
            .iter()
            .find(|x| x.name.to_ascii_lowercase().as_slice() == b"boundary")
            .map(|boundary| Multipart {
                subtype: MultipartSubtype::from(nt),
                boundary: boundary.value.to_string(),
            })
            .ok_or(())
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum MultipartSubtype {
    Alternative,
    Mixed,
    Digest,
    Parallel,
    Report,
    Unknown,
}
impl ToString for MultipartSubtype {
    fn to_string(&self) -> String {
        match self {
            Self::Alternative => "alternative",
            Self::Mixed => "mixed",
            Self::Digest => "digest",
            Self::Parallel => "parallel",
            Self::Report => "report",
            Self::Unknown => "mixed",
        }
        .into()
    }
}
impl<'a> From<&NaiveType<'a>> for MultipartSubtype {
    fn from(nt: &NaiveType<'a>) -> Self {
        match nt.sub.to_ascii_lowercase().as_slice() {
            b"alternative" => Self::Alternative,
            b"mixed" => Self::Mixed,
            b"digest" => Self::Digest,
            b"parallel" => Self::Parallel,
            b"report" => Self::Report,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, PartialEq, Default, Clone)]
pub enum MessageSubtype {
    #[default]
    RFC822,
    Partial,
    External,
    Unknown,
}
impl ToString for MessageSubtype {
    fn to_string(&self) -> String {
        match self {
            Self::RFC822 => "rfc822",
            Self::Partial => "partial",
            Self::External => "external",
            Self::Unknown => "rfc822",
        }
        .into()
    }
}

pub type DeductibleMessage = Deductible<Message>;
#[derive(Debug, PartialEq, Default, Clone)]
pub struct Message {
    pub subtype: MessageSubtype,
}
impl<'a> From<&NaiveType<'a>> for Message {
    fn from(nt: &NaiveType<'a>) -> Self {
        match nt.sub.to_ascii_lowercase().as_slice() {
            b"rfc822" => Self {
                subtype: MessageSubtype::RFC822,
            },
            b"partial" => Self {
                subtype: MessageSubtype::Partial,
            },
            b"external" => Self {
                subtype: MessageSubtype::External,
            },
            _ => Self {
                subtype: MessageSubtype::Unknown,
            },
        }
    }
}
impl From<Deductible<Message>> for Message {
    fn from(d: Deductible<Message>) -> Self {
        match d {
            Deductible::Inferred(t) | Deductible::Explicit(t) => t,
        }
    }
}

pub type DeductibleText = Deductible<Text>;
#[derive(Debug, PartialEq, Default, Clone)]
pub struct Text {
    pub subtype: TextSubtype,
    pub charset: Deductible<EmailCharset>,
}
impl<'a> From<&NaiveType<'a>> for Text {
    fn from(nt: &NaiveType<'a>) -> Self {
        Self {
            subtype: TextSubtype::from(nt),
            charset: nt
                .params
                .iter()
                .find(|x| x.name.to_ascii_lowercase().as_slice() == b"charset")
                .map(|x| Deductible::Explicit(EmailCharset::from(x.value.to_string().as_bytes())))
                .unwrap_or(Deductible::Inferred(EmailCharset::US_ASCII)),
        }
    }
}
impl From<Deductible<Text>> for Text {
    fn from(d: Deductible<Text>) -> Self {
        match d {
            Deductible::Inferred(t) | Deductible::Explicit(t) => t,
        }
    }
}

#[derive(Debug, PartialEq, Default, Clone)]
pub enum TextSubtype {
    #[default]
    Plain,
    Html,
    Unknown,
}
impl ToString for TextSubtype {
    fn to_string(&self) -> String {
        match self {
            Self::Plain | Self::Unknown => "plain",
            Self::Html => "html",
        }
        .into()
    }
}
impl<'a> From<&NaiveType<'a>> for TextSubtype {
    fn from(nt: &NaiveType<'a>) -> Self {
        match nt.sub.to_ascii_lowercase().as_slice() {
            b"plain" => Self::Plain,
            b"html" => Self::Html,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, PartialEq, Default, Clone)]
pub struct Binary {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mime::charset::EmailCharset;
    use crate::mime::r#type::Deductible;
    use crate::text::quoted::QuotedString;

    #[test]
    fn test_parameter() {
        assert_eq!(
            parameter(b"charset=utf-8"),
            Ok((
                &b""[..],
                Parameter {
                    name: &b"charset"[..],
                    value: MIMEWord::Atom(&b"utf-8"[..]),
                }
            )),
        );
        assert_eq!(
            parameter(b"charset=\"utf-8\""),
            Ok((
                &b""[..],
                Parameter {
                    name: &b"charset"[..],
                    value: MIMEWord::Quoted(QuotedString(vec![&b"utf-8"[..]])),
                }
            )),
        );
    }

    #[test]
    fn test_content_type_plaintext() {
        let (rest, nt) = naive_type(b"text/plain;\r\n charset=utf-8").unwrap();
        assert_eq!(rest, &b""[..]);

        assert_eq!(
            nt.to_type(),
            AnyType::Text(Deductible::Explicit(Text {
                charset: Deductible::Explicit(EmailCharset::UTF_8),
                subtype: TextSubtype::Plain,
            }))
        );
    }

    #[test]
    fn test_content_type_multipart() {
        let (rest, nt) = naive_type(b"multipart/mixed;\r\n\tboundary=\"--==_mimepart_64a3f2c69114f_2a13d020975fe\";\r\n\tcharset=UTF-8").unwrap();
        assert_eq!(rest, &[]);
        assert_eq!(
            nt.to_type(),
            AnyType::Multipart(Multipart {
                subtype: MultipartSubtype::Mixed,
                boundary: "--==_mimepart_64a3f2c69114f_2a13d020975fe".into(),
            })
        );
    }

    #[test]
    fn test_content_type_message() {
        let (rest, nt) = naive_type(b"message/rfc822").unwrap();
        assert_eq!(rest, &[]);

        assert_eq!(
            nt.to_type(),
            AnyType::Message(Deductible::Explicit(Message {
                subtype: MessageSubtype::RFC822
            }))
        );
    }

    #[test]
    fn test_parameter_ascii() {
        assert_eq!(
            parameter(b"charset = (simple) us-ascii (Plain text)"),
            Ok((
                &b""[..],
                Parameter {
                    name: &b"charset"[..],
                    value: MIMEWord::Atom(&b"us-ascii"[..]),
                }
            ))
        );
    }

    #[test]
    fn test_parameter_terminated_with_semi_colon() {
        assert_eq!(
            parameter_list(b";boundary=\"festivus\";"),
            Ok((
                &b""[..],
                vec![Parameter {
                    name: &b"boundary"[..],
                    value: MIMEWord::Quoted(QuotedString(vec![&b"festivus"[..]])),
                }],
            ))
        );
    }
}
