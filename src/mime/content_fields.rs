use std::borrow::Cow;
use encoding_rs::Encoding;
use nom::{
    branch::alt,
    bytes::complete::{tag,take_while1}, 
    character::complete as character, 
    combinator::{into, opt}, 
    multi::many0,
    sequence::{delimited, preceded, tuple},
    IResult,
};

use crate::error::IMFError;
use crate::fragments::lazy;
use crate::fragments::whitespace::cfws;
use crate::fragments::quoted::quoted_string;





/*
impl<'a> From<&'a str> for MultipartSubtype<'a> {
    fn from(csub: &'a str) -> Self {
        match csub.to_lowercase().as_ref() {
            "alternative" => MultipartSubtype::Alternative,
            "mixed" => MultipartSubtype::Mixed,
            "digest" => MultipartSubtype::Digest,
            "parallel" => MultipartSubtype::Parallel,
            "report" => MultipartSubtype::Report,
            _ => MultipartSubtype::Other(csub),
        }
    }
}

impl<'a> From<&'a str> for MessageSubtype<'a> {
    fn from(csub: &'a str) -> Self {
        match csub.to_lowercase().as_ref() {
            "rfc822" => MessageSubtype::RFC822,
            "partial" => MessageSubtype::Partial,
            "external" => MessageSubtype::External,
            _ => MessageSubtype::Other(csub),
        }
    }
}

impl<'a> From<&'a str> for TextSubtype<'a> {
    fn from(csub: &'a str) -> Self {
        match csub.to_lowercase().as_ref() {
            "html" => TextSubtype::Html,
            "plain" => TextSubtype::Plain,
            _ => TextSubtype::Other(csub),
        }
    }
}
*/

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fragments::lazy;

    #[test]
    fn test_version() {
        assert_eq!(version("1.0"), Ok(("", Version { major: 1, minor: 0 })),);

        assert_eq!(
            version(" 1.0 (produced by MetaSend Vx.x)"),
            Ok(("", Version { major: 1, minor: 0 })),
        );

        assert_eq!(
            version("(produced by MetaSend Vx.x) 1.0"),
            Ok(("", Version { major: 1, minor: 0 })),
        );

        assert_eq!(
            version("1.(produced by MetaSend Vx.x)0"),
            Ok(("", Version { major: 1, minor: 0 })),
        );
    }

    #[test]
    fn test_parameter() {
        assert_eq!(
            parameter("charset=utf-8"),
            Ok(("", Parameter::Charset(EmailCharset::UTF_8))),
        );
        assert_eq!(
            parameter("charset=\"utf-8\""),
            Ok(("", Parameter::Charset(EmailCharset::UTF_8))),
        );
    }

    #[test]
    fn test_content_type_plaintext() {
        assert_eq!(
            Type::try_from(&lazy::Type("text/plain; charset=utf-8")),
            Ok(Type::Text(TextDesc {
                charset: Some(EmailCharset::UTF_8),
                subtype: TextSubtype::Plain,
                unknown_parameters: vec![],
            }))
        );
    }

    #[test]
    fn test_content_type_multipart() {
        assert_eq!(
            Type::try_from(&lazy::Type("multipart/mixed;\r\n\tboundary=\"--==_mimepart_64a3f2c69114f_2a13d020975fe\";\r\n\tcharset=UTF-8")),
            Ok(Type::Multipart(MultipartDesc {
                subtype: MultipartSubtype::Mixed,
                boundary: "--==_mimepart_64a3f2c69114f_2a13d020975fe".into(),
                unknown_parameters: vec![Parameter::Charset(EmailCharset::UTF_8)],
            }))
        );
    }

    #[test]
    fn test_content_type_message() {
        assert_eq!(
            Type::try_from(&lazy::Type("message/rfc822")),
            Ok(Type::Message(MessageDesc {
                subtype: MessageSubtype::RFC822,
                unknown_parameters: vec![],
            }))
        );
    }

    #[test]
    fn test_parameter_ascii() {
        assert_eq!(
            parameter("charset=us-ascii (Plain text)"),
            Ok(("", Parameter::Charset(EmailCharset::US_ASCII)))
        );
    }


}
