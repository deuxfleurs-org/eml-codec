use bounded_static::ToStatic;
use nom::{
    bytes::complete::tag,
    combinator::{map, opt},
    multi::many0,
    sequence::{preceded, terminated, tuple},
    IResult,
};
use std::borrow::Cow;
use std::fmt;

use crate::display_bytes::{Print, Formatter};
use crate::mime::charset::EmailCharset;
use crate::text::misc_token::{mime_word, MIMEWord};
use crate::text::words::mime_atom;

// --------- NAIVE TYPE
#[derive(PartialEq, Clone, ToStatic)]
pub struct NaiveType<'a> {
    pub main: Cow<'a, [u8]>,
    pub sub: Cow<'a, [u8]>,
    pub params: Vec<Parameter<'a>>,
}
impl<'a> fmt::Debug for NaiveType<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("mime::NaiveType")
            .field("main", &String::from_utf8_lossy(&self.main))
            .field("sub", &String::from_utf8_lossy(&self.sub))
            .field("params", &self.params)
            .finish()
    }
}
impl<'a> NaiveType<'a> {
    pub fn to_type(&self) -> AnyType {
        self.into()
    }
}
pub fn naive_type(input: &[u8]) -> IResult<&[u8], NaiveType<'_>> {
    map(
        tuple((mime_atom, tag("/"), mime_atom, parameter_list)),
        |(main, _, sub, params)| NaiveType {
            main: Cow::Borrowed(main),
            sub: Cow::Borrowed(sub),
            params,
        },
    )(input)
}

#[derive(PartialEq, Clone, ToStatic)]
pub struct Parameter<'a> {
    pub name: Cow<'a, [u8]>,
    pub value: MIMEWord<'a>,
}
impl<'a> fmt::Debug for Parameter<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("mime::Parameter")
            .field("name", &String::from_utf8_lossy(&self.name))
            .field("value", &self.value)
            .finish()
    }
}

pub fn parameter(input: &[u8]) -> IResult<&[u8], Parameter<'_>> {
    map(
        tuple((mime_atom, tag(b"="), mime_word)),
        |(name, _, value)| Parameter {
            name: Cow::Borrowed(name),
            value,
        },
    )(input)
}
// XXX the final optional ; is not specified in RFC2045
pub fn parameter_list(input: &[u8]) -> IResult<&[u8], Vec<Parameter<'_>>> {
    terminated(many0(preceded(tag(";"), parameter)), opt(tag(";")))(input)
}

// MIME TYPES TRANSLATED TO RUST TYPING SYSTEM

#[derive(Debug, PartialEq, ToStatic)]
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

impl Print for AnyType {
    fn print(&self, fmt: &mut impl Formatter) -> std::io::Result<()> {
        match self {
            AnyType::Multipart(mp) => {
                fmt.write_bytes(b"multipart/")?;
                fmt.write_bytes(mp.subtype.to_string().as_bytes())?;
                fmt.write_bytes(b";")?;
                fmt.write_fws()?;
                // always quote the boundary ("never hurts" says RFC2046)
                fmt.write_bytes(b"boundary=\"")?;
                fmt.write_bytes(&mp.boundary)?;
                fmt.write_bytes(b"\"")
            },
            AnyType::Message(msg) => {
                fmt.write_bytes(b"message/")?;
                fmt.write_bytes(msg.value().subtype.to_string().as_bytes())
            },
            AnyType::Text(txt) => {
                fmt.write_bytes(b"text/")?;
                fmt.write_bytes(txt.value().subtype.to_string().as_bytes())?;
                fmt.write_bytes(b";")?;
                fmt.write_fws()?;
                fmt.write_bytes(b"charset=")?;
                fmt.write_bytes(txt.value().charset.value().as_str().as_bytes())
            },
            AnyType::Binary(_bin) => {
                fmt.write_bytes(b"application/octet-stream")
            }
        }
    }
}

#[derive(Debug, PartialEq, Clone, ToStatic)]
pub enum Deductible<T: Default> {
    Inferred(T),
    Explicit(T),
}
impl<T: Default> Default for Deductible<T> {
    fn default() -> Self {
        Self::Inferred(T::default())
    }
}
impl<T: Default> Deductible<T> {
    fn value(&self) -> &T {
        match self {
            Deductible::Inferred(x) => x,
            Deductible::Explicit(x) => x,
        }
    }
}

// REAL PARTS

#[derive(Debug, PartialEq, Clone, ToStatic)]
pub struct Multipart {
    pub subtype: MultipartSubtype,
    pub boundary: Vec<u8>,
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
                boundary: boundary.value.bytes().collect(),
            })
            .ok_or(())
    }
}

#[derive(Debug, PartialEq, Clone, ToStatic)]
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

#[derive(Debug, PartialEq, Default, Clone, ToStatic)]
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
#[derive(Debug, PartialEq, Default, Clone, ToStatic)]
pub struct Message {
    // XXX no parameters? required for 'partial' and 'external'
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
#[derive(Debug, PartialEq, Default, Clone, ToStatic)]
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

#[derive(Debug, PartialEq, Default, Clone, ToStatic)]
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

#[derive(Debug, PartialEq, Default, Clone, ToStatic)]
pub struct Binary {
    // XXX forward content types even if not interpreted?
}

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
                    name: b"charset"[..].into(),
                    value: MIMEWord::Atom(b"utf-8"[..].into()),
                }
            )),
        );
        assert_eq!(
            parameter(b"charset=\"utf-8\""),
            Ok((
                &b""[..],
                Parameter {
                    name: b"charset"[..].into(),
                    value: MIMEWord::Quoted(QuotedString(vec![b"utf-8"[..].into()])),
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
    fn test_content_type_comment() {
        let (rest, nt) = naive_type(b"text/plain; charset=\"us-ascii\" (Plain text)").unwrap();
        assert_eq!(rest, &[]);

        assert_eq!(
            nt.to_type(),
            AnyType::Text(Deductible::Explicit(Text {
                subtype: TextSubtype::Plain,
                charset: Deductible::Explicit(EmailCharset::US_ASCII),
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
                    name: b"charset"[..].into(),
                    value: MIMEWord::Atom(b"us-ascii"[..].into()),
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
                    name: b"boundary"[..].into(),
                    value: MIMEWord::Quoted(QuotedString(vec![b"festivus"[..].into()])),
                }],
            ))
        );
    }
}
