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

use crate::print::{Print, Formatter};
use crate::mime::charset::EmailCharset;
use crate::text::misc_token::{mime_word, MIMEWord};
use crate::text::words::mime_atom;
use crate::utils::Deductible;

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
    pub fn to_type(&self) -> AnyType<'a> {
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

// XXX we allow printing content types without further validation;
// this is not strictly allowed by the spec, which only allows
// x-token or ietf-token on top of the RFC defined content types.
impl<'a> Print for NaiveType<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        self.main.print(fmt);
        fmt.write_bytes(b"/");
        self.sub.print(fmt);
        for param in &self.params {
            fmt.write_bytes(b";");
            fmt.write_fws();
            param.print(fmt);
        }
    }
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
impl<'a> Print for Parameter<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        fmt.write_bytes(&self.name);
        fmt.write_bytes(b"=");
        self.value.print(fmt)
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
pub enum AnyType<'a> {
    // Composite types
    Multipart(Multipart<'a>),         // multipart/*
    Message(Message<'a>), // message/*

    // Discrete types
    Text(Deductible<Text<'a>>),       // text/*
    Binary(Binary<'a>),               // everything else
}

impl<'a> From<&NaiveType<'a>> for AnyType<'a> {
    fn from(nt: &NaiveType<'a>) -> Self {
        match nt.main.to_ascii_lowercase().as_slice() {
            b"multipart" =>
                 // fails if there is no boundary parameter
                Multipart::try_from(nt)
                .map(Self::Multipart)
                .unwrap_or(Self::Binary(Binary::from(nt))),
            b"message" => Self::Message(Message::from(nt)),
            b"text" => Self::Text(DeductibleText::Explicit(Text::from(nt))),
            _ => Self::Binary(Binary::from(nt)),
        }
    }
}

impl<'a> Print for AnyType<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        match self {
            AnyType::Multipart(mp) => mp.print(fmt),
            AnyType::Message(msg) => msg.print(fmt),
            AnyType::Text(txt) => txt.print(fmt),
            AnyType::Binary(bin) => bin.print(fmt),
        }
    }
}

// REAL PARTS

#[derive(PartialEq, Clone, ToStatic)]
pub struct Multipart<'a> {
    pub subtype: MultipartSubtype,
    // XXX: this is a hack, it is used to propagate information during parsing,
    // but is ignored by the printer.
    pub boundary: Option<Vec<u8>>,
    pub params: Vec<Parameter<'a>>,
}

impl<'a> fmt::Debug for Multipart<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("mime::r#type::Multipart")
            .field("subtype", &self.subtype)
            .field("boundary", &self.boundary.as_deref().map(String::from_utf8_lossy))
            .field("params", &self.params)
            .finish()
    }
}

impl<'a> Print for Multipart<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        fmt.push_new_boundary();
        fmt.write_bytes(b"multipart/");
        self.subtype.print(fmt);
        fmt.write_bytes(b";");
        fmt.write_fws();
        // always quote the boundary ("never hurts" says RFC2046)
        fmt.write_bytes(b"boundary=\"");
        fmt.write_current_boundary();
        fmt.write_bytes(b"\"");
        for param in &self.params {
            fmt.write_bytes(b";");
            fmt.write_fws();
            param.print(fmt);
        }
    }
}

impl<'a> TryFrom<&NaiveType<'a>> for Multipart<'a> {
    type Error = ();

    fn try_from(nt: &NaiveType<'a>) -> Result<Self, Self::Error> {
        let mut params = vec![];
        let mut boundary = None;
        for param in &nt.params {
            if param.name.to_ascii_lowercase().as_slice() == b"boundary" {
                if boundary.is_none() {
                    boundary = Some(param.value.bytes().collect::<Vec<_>>());
                }
                // drop any redundant "boundary" parameter that is not the first
            } else {
                params.push(param.clone())
            }
        }
        match boundary {
            Some(boundary) => Ok(Multipart {
                subtype: MultipartSubtype::from(nt),
                boundary: Some(boundary),
                params,
            }),
            None => Err(()),
        }
    }
}

#[derive(Debug, PartialEq, Clone, ToStatic)]
pub enum MultipartSubtype {
    Alternative,
    Mixed,
    Digest,
    Parallel,
    Report,
    Unknown(Vec<u8>), // should be treated as Mixed
}
impl MultipartSubtype {
    fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Alternative => b"alternative",
            Self::Mixed => b"mixed",
            Self::Digest => b"digest",
            Self::Parallel => b"parallel",
            Self::Report => b"report",
            Self::Unknown(v) => &v,
        }
    }
}
impl ToString for MultipartSubtype {
    fn to_string(&self) -> String {
        String::from_utf8_lossy(self.as_bytes()).to_string()
    }
}
impl Print for MultipartSubtype {
    fn print(&self, fmt: &mut impl Formatter) {
        fmt.write_bytes(self.as_bytes())
    }
}

impl<'a> From<&NaiveType<'a>> for MultipartSubtype {
    fn from(nt: &NaiveType<'a>) -> Self {
        let sub = nt.sub.to_ascii_lowercase();
        match sub.as_slice() {
            b"alternative" => Self::Alternative,
            b"mixed" => Self::Mixed,
            b"digest" => Self::Digest,
            b"parallel" => Self::Parallel,
            b"report" => Self::Report,
            _ => Self::Unknown(sub),
        }
    }
}

#[derive(Debug, PartialEq, Default, Clone, ToStatic)]
pub enum MessageSubtype {
    #[default]
    RFC822,
    Partial,
    External,
    Unknown(Vec<u8>), // should be treated as the Binary type
}
impl MessageSubtype {
    fn as_bytes(&self) -> &[u8] {
        match self {
            Self::RFC822 => b"rfc822",
            Self::Partial => b"partial",
            Self::External => b"external",
            Self::Unknown(b) => &b,
        }
    }
}
impl ToString for MessageSubtype {
    fn to_string(&self) -> String {
        String::from_utf8_lossy(self.as_bytes()).to_string()
    }
}
impl Print for MessageSubtype {
    fn print(&self, fmt: &mut impl Formatter) {
        fmt.write_bytes(self.as_bytes())
    }
}

impl<'a> From<&NaiveType<'a>> for MessageSubtype {
    fn from(nt: &NaiveType<'a>) -> Self {
        let sub = nt.sub.to_ascii_lowercase();
        match sub.as_slice() {
            b"rfc822" => MessageSubtype::RFC822,
            b"partial" => MessageSubtype::Partial,
            b"external" => MessageSubtype::External,
            _ => MessageSubtype::Unknown(sub),
        }
    }
}

#[derive(Debug, PartialEq, Default, Clone, ToStatic)]
pub struct Message<'a> {
    pub subtype: MessageSubtype,
    pub params: Vec<Parameter<'a>>,
}

impl<'a> Print for Message<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        fmt.write_bytes(b"message/");
        self.subtype.print(fmt);
        for param in &self.params {
            fmt.write_bytes(b";");
            fmt.write_fws();
            param.print(fmt);
        }
    }
}

impl<'a> From<&NaiveType<'a>> for Message<'a> {
    fn from(nt: &NaiveType<'a>) -> Self {
        Self {
            subtype: MessageSubtype::from(nt),
            params: nt.params.clone(),
        }
    }
}

pub type DeductibleText<'a> = Deductible<Text<'a>>;
#[derive(Debug, PartialEq, Default, Clone, ToStatic)]
pub struct Text<'a> {
    // NOTE: an unknown subtype combined with an unknown charset should
    // result in this type be treated as equivalent to the Binary type.
    pub subtype: TextSubtype,
    pub charset: Deductible<EmailCharset>,
    pub params: Vec<Parameter<'a>>,
}

impl<'a> Print for Text<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        fmt.write_bytes(b"text/");
        self.subtype.print(fmt);
        fmt.write_bytes(b";");
        fmt.write_fws();
        fmt.write_bytes(b"charset=");
        fmt.write_bytes(self.charset.value().as_bytes());
        for param in &self.params {
            fmt.write_bytes(b";");
            fmt.write_fws();
            param.print(fmt);
        }
    }
}

impl<'a> From<&NaiveType<'a>> for Text<'a> {
    fn from(nt: &NaiveType<'a>) -> Self {
        let mut params = vec![];
        let mut charset = Deductible::Inferred(EmailCharset::US_ASCII);
        for param in &nt.params {
            if param.name.to_ascii_lowercase().as_slice() == b"charset" {
                if matches!(charset, Deductible::Inferred(_)) {
                    let value: Vec<u8> = param.value.bytes().collect();
                    charset = Deductible::Explicit(EmailCharset::from(value.as_slice()));
                }
                // drop any "charset" parameter that is not the first
            } else {
                params.push(param.clone())
            }
        }

        Self { subtype: TextSubtype::from(nt), charset, params }
    }
}

#[derive(Debug, PartialEq, Default, Clone, ToStatic)]
pub enum TextSubtype {
    #[default]
    Plain,
    Html,
    Unknown(Vec<u8>),
}
impl TextSubtype {
    fn as_bytes(&self) -> &[u8] {
        use TextSubtype::*;
        match self {
            Plain => b"plain",
            Html => b"html",
            Unknown(b) => &b,
        }
    }
}
impl ToString for TextSubtype {
    fn to_string(&self) -> String {
        String::from_utf8_lossy(self.as_bytes()).to_string()
    }
}
impl Print for TextSubtype {
    fn print(&self, fmt: &mut impl Formatter) {
        fmt.write_bytes(self.as_bytes())
    }
}

impl<'a> From<&NaiveType<'a>> for TextSubtype {
    fn from(nt: &NaiveType<'a>) -> Self {
        let sub = nt.sub.to_ascii_lowercase();
        match sub.as_slice() {
            b"plain" => Self::Plain,
            b"html" => Self::Html,
            _ => Self::Unknown(sub),
        }
    }
}

#[derive(Debug, PartialEq, Clone, ToStatic)]
pub struct Binary<'a> {
    ctype: NaiveType<'a>,
}

impl<'a> Print for Binary<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        self.ctype.print(fmt)
    }
}
impl<'a> From<&NaiveType<'a>> for Binary<'a> {
    fn from(nt: &NaiveType<'a>) -> Self {
        Self { ctype: nt.clone() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mime::charset::EmailCharset;
    use crate::utils::Deductible;
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
        let (rest, nt) = naive_type(b"text/plain;\r\n charset=utf-8 ; hello=yolo").unwrap();
        assert_eq!(rest, &b""[..]);

        assert_eq!(
            nt.to_type(),
            AnyType::Text(Deductible::Explicit(Text {
                charset: Deductible::Explicit(EmailCharset::UTF_8),
                subtype: TextSubtype::Plain,
                params: vec![Parameter {
                    name: b"hello"[..].into(),
                    value: MIMEWord::Atom(b"yolo"[..].into()),
                }],
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
                boundary: Some("--==_mimepart_64a3f2c69114f_2a13d020975fe".into()),
                params: vec![Parameter {
                    name: b"charset"[..].into(),
                    value: MIMEWord::Atom(b"UTF-8"[..].into()),
                }],
            })
        );
    }

    #[test]
    fn test_content_type_message() {
        let (rest, nt) = naive_type(b"message/rfc822").unwrap();
        assert_eq!(rest, &[]);

        assert_eq!(
            nt.to_type(),
            AnyType::Message(Message {
                subtype: MessageSubtype::RFC822,
                params: vec![],
            })
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
                params: vec![],
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
