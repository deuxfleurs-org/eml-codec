use nom::{
    bytes::complete::tag, 
    combinator::map, 
    multi::many0,
    sequence::{preceded, tuple},
    IResult,
};

use crate::mime::charset::EmailCharset;
use crate::text::misc_token::{MIMEWord, mime_word};
use crate::text::words::{mime_atom};

// --------- NAIVE TYPE
#[derive(Debug, PartialEq)]
pub struct NaiveType<'a> {
    main: &'a [u8],
    sub: &'a [u8],
    params: Vec<Parameter<'a>>,
}
impl<'a> NaiveType<'a> {
    pub fn to_type(&self) -> Type { self.into() } 
}
pub fn naive_type(input: &[u8]) -> IResult<&[u8], NaiveType> {
    map(
        tuple((mime_atom, tag("/"), mime_atom, parameter_list)),
        |(main, _, sub, params)| NaiveType { main, sub, params },
    )(input)
}

#[derive(Debug, PartialEq)]
pub struct Parameter<'a> {
    name: &'a [u8],
    value: MIMEWord<'a>,
}
pub fn parameter(input: &[u8]) -> IResult<&[u8], Parameter> {
    map(tuple((mime_atom, tag(b"="), mime_word)), |(name, _, value)| Parameter { name, value })(input)
}
pub fn parameter_list(input: &[u8]) -> IResult<&[u8], Vec<Parameter>> {
    many0(preceded(tag(";"), parameter))(input)
}

// -------- TYPE
#[derive(Debug, PartialEq)]
pub enum Type {
    // Composite types
    Multipart(MultipartDesc),
    Message(MessageSubtype),

    // Discrete types
    Text(TextDesc),
    Binary,
}
impl Default for Type {
    fn default() -> Self {
        Self::Text(TextDesc::default())
    }
}
impl<'a> From<&'a NaiveType<'a>> for Type {
    fn from(nt: &'a NaiveType<'a>) -> Self {
        match nt.main.to_ascii_lowercase().as_slice() {
            b"multipart" => MultipartDesc::try_from(nt).map(Self::Multipart).unwrap_or(Self::default()),
            b"message" => Self::Message(MessageSubtype::from(nt)),
            b"text" => Self::Text(TextDesc::from(nt)),
            _ => Self::Binary,
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct MultipartDesc {
    pub subtype: MultipartSubtype,
    pub boundary: String,
}
impl<'a> TryFrom<&'a NaiveType<'a>> for MultipartDesc {
    type Error = ();

    fn try_from(nt: &'a NaiveType<'a>) -> Result<Self, Self::Error> {
        nt.params.iter()
            .find(|x| x.name.to_ascii_lowercase().as_slice() == b"boundary")
            .map(|boundary| MultipartDesc {
                subtype: MultipartSubtype::from(nt),
                boundary: boundary.value.to_string(),
            })
            .ok_or(())
    }
}

#[derive(Debug, PartialEq)]
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

#[derive(Debug, PartialEq)]
pub enum MessageSubtype {
    RFC822,
    Partial,
    External,
    Unknown,
}
impl<'a> From<&NaiveType<'a>> for MessageSubtype {
    fn from(nt: &NaiveType<'a>) -> Self {
        match nt.sub.to_ascii_lowercase().as_slice() {
            b"rfc822" => Self::RFC822,
            b"partial" => Self::Partial,
            b"external" => Self::External,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, PartialEq, Default)]
pub struct TextDesc {
    pub subtype: TextSubtype,
    pub charset: EmailCharset,
}
impl<'a> From<&NaiveType<'a>> for TextDesc {
    fn from(nt: &NaiveType<'a>) -> Self {
        TextDesc {
            subtype: TextSubtype::from(nt),
            charset: nt.params.iter()
                .find(|x| x.name.to_ascii_lowercase().as_slice() == b"charset")
                .map(|x| EmailCharset::from(x.value.to_string().as_bytes()))
                .unwrap_or(EmailCharset::US_ASCII),
        }
    }
}

#[derive(Debug, PartialEq, Default)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::quoted::QuotedString;

    #[test]
    fn test_parameter() {
        assert_eq!(
            parameter(b"charset=utf-8"),
            Ok((&b""[..], Parameter { 
                name: &b"charset"[..], 
                value: MIMEWord::Atom(&b"utf-8"[..]), 
            })),
        );
        assert_eq!(
            parameter(b"charset=\"utf-8\""),
            Ok((&b""[..], Parameter {
                name: &b"charset"[..],
                value: MIMEWord::Quoted(QuotedString(vec![&b"utf-8"[..]])),
            })),
        );
    }

    #[test]
    fn test_content_type_plaintext() {
        let (rest, nt) = naive_type(b"text/plain;\r\n charset=utf-8").unwrap();
        assert_eq!(rest, &b""[..]);

        assert_eq!(
            nt.to_type(), 
            Type::Text(TextDesc {
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
            Type::Multipart(MultipartDesc {
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
            Type::Message(MessageSubtype::RFC822),
        );
    }

    #[test]
    fn test_parameter_ascii() {
        assert_eq!(
            parameter(b"charset = (simple) us-ascii (Plain text)"),
            Ok((&b""[..], Parameter {
                name: &b"charset"[..],
                value: MIMEWord::Atom(&b"us-ascii"[..]),
            }))
        );
    }
}
