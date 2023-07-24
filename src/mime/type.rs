use nom::{
    bytes::complete::tag,
    combinator::{map, opt},
    multi::many0,
    sequence::{preceded, terminated, tuple},
    IResult,
};

use crate::mime::charset::EmailCharset;
use crate::text::misc_token::{mime_word, MIMEWord};
use crate::text::words::mime_atom;
use crate::mime::{AnyMIME, MIME, NaiveMIME};

// --------- NAIVE TYPE
#[derive(Debug, PartialEq, Clone)]
pub struct NaiveType<'a> {
    pub main: &'a [u8],
    pub sub: &'a [u8],
    pub params: Vec<Parameter<'a>>,
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

#[derive(Debug, PartialEq, Clone)]
pub struct Parameter<'a> {
    pub name: &'a [u8],
    pub value: MIMEWord<'a>,
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
    Message(Message),

    // Discrete types
    Text(Text),
    Binary(Binary),
}

impl<'a> From<&'a NaiveType<'a>> for AnyType {
    fn from(nt: &'a NaiveType<'a>) -> Self {
        match nt.main.to_ascii_lowercase().as_slice() {
            b"multipart" => Multipart::try_from(nt)
                .map(Self::Multipart)
                .unwrap_or(Self::Text(Text::default())),
            b"message" => Self::Message(Message::from(nt)),
            b"text" => Self::Text(Text::from(nt)),
            _ => Self::Binary(Binary::default()),
        }
    }
}

impl<'a> AnyType {
    pub fn to_mime(self, parsed: NaiveMIME<'a>) -> AnyMIME<'a> {
         match self {
            Self::Multipart(interpreted) => AnyMIME::Mult(MIME::<Multipart> { interpreted, parsed }),
            Self::Message(interpreted) => AnyMIME::Msg(MIME::<Message> { interpreted, parsed }),
            Self::Text(interpreted) => AnyMIME::Txt(MIME::<Text> { interpreted, parsed }),
            Self::Binary(interpreted) => AnyMIME::Bin(MIME::<Binary> { interpreted, parsed }),
        }       
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Multipart {
    pub subtype: MultipartSubtype,
    pub boundary: String,
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
pub enum Message {
    #[default]
    RFC822,
    Partial,
    External,
    Unknown,
}
impl<'a> From<&NaiveType<'a>> for Message {
    fn from(nt: &NaiveType<'a>) -> Self {
        match nt.sub.to_ascii_lowercase().as_slice() {
            b"rfc822" => Self::RFC822,
            b"partial" => Self::Partial,
            b"external" => Self::External,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, PartialEq, Default, Clone)]
pub struct Text {
    pub subtype: TextSubtype,
    pub charset: EmailCharset,
}
impl<'a> From<&NaiveType<'a>> for Text {
    fn from(nt: &NaiveType<'a>) -> Self {
        Self {
            subtype: TextSubtype::from(nt),
            charset: nt
                .params
                .iter()
                .find(|x| x.name.to_ascii_lowercase().as_slice() == b"charset")
                .map(|x| EmailCharset::from(x.value.to_string().as_bytes()))
                .unwrap_or(EmailCharset::US_ASCII),
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
            AnyType::Text(Text {
                charset: EmailCharset::UTF_8,
                subtype: TextSubtype::Plain,
            })
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

        assert_eq!(nt.to_type(), AnyType::Message(Message::RFC822),);
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
