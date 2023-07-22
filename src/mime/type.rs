use nom::{
    bytes::complete::tag, 
    combinator::map, 
    multi::many0,
    sequence::{preceded, tuple},
    IResult,
};

use crate::text::misc_token::{MIMEWord, mime_word};
use crate::text::words::{mime_atom};
use crate::mime::mime::{Type};

// --------- NAIVE TYPE
#[derive(Debug, PartialEq)]
pub struct NaiveType<'a> {
    pub main: &'a [u8],
    pub sub: &'a [u8],
    pub params: Vec<Parameter<'a>>,
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
    pub name: &'a [u8],
    pub value: MIMEWord<'a>,
}
pub fn parameter(input: &[u8]) -> IResult<&[u8], Parameter> {
    map(tuple((mime_atom, tag(b"="), mime_word)), |(name, _, value)| Parameter { name, value })(input)
}
pub fn parameter_list(input: &[u8]) -> IResult<&[u8], Vec<Parameter>> {
    many0(preceded(tag(";"), parameter))(input)
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::mime::charset::EmailCharset;
    use crate::text::quoted::QuotedString;
    use crate::mime::mime::*;

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
            Type::Text(Text {
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
            Type::Multipart(Multipart {
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
            Type::Message(Message::RFC822),
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
