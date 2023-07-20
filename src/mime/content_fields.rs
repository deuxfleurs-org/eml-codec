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

#[derive(Debug, PartialEq)]
pub enum Type<'a> {
    // Composite types
    Multipart(MultipartDesc<'a>),
    Message(MessageDesc<'a>),

    // Discrete types
    Text(TextDesc<'a>),
    Image(&'a str, Vec<Parameter<'a>>),
    Audio(&'a str, Vec<Parameter<'a>>),
    Video(&'a str, Vec<Parameter<'a>>),
    Application(&'a str, Vec<Parameter<'a>>),

    // Unknown
    Other(&'a str, &'a str, Vec<Parameter<'a>>),
}

#[derive(Debug, PartialEq)]
pub struct MultipartDesc<'a> {
    pub boundary: String,
    pub subtype: MultipartSubtype<'a>,
    pub unknown_parameters: Vec<Parameter<'a>>,
}

#[derive(Debug, PartialEq)]
pub enum MultipartSubtype<'a> {
    Alternative,
    Mixed,
    Digest,
    Parallel,
    Report,
    Other(&'a str),
}

#[derive(Debug, PartialEq)]
pub struct MessageDesc<'a> {
    pub subtype: MessageSubtype<'a>,
    pub unknown_parameters: Vec<Parameter<'a>>,
}

#[derive(Debug, PartialEq)]
pub enum MessageSubtype<'a> {
    RFC822,
    Partial,
    External,
    Other(&'a str),
}

#[derive(Debug, PartialEq)]
pub struct TextDesc<'a> {
    pub charset: Option<EmailCharset<'a>>,
    pub subtype: TextSubtype<'a>,
    pub unknown_parameters: Vec<Parameter<'a>>,
}

#[derive(Debug, PartialEq)]
pub enum TextSubtype<'a> {
    Plain,
    Html,
    Other(&'a str),
}

#[derive(Debug, PartialEq)]
pub enum Parameter<'a> {
    Charset(EmailCharset<'a>),
    Boundary(String),
    Other(&'a str, String),
}

#[derive(Debug, PartialEq)]
pub enum Mechanism<'a> {
    _7Bit,
    _8Bit,
    Binary,
    QuotedPrintable,
    Base64,
    Other(&'a str),
}





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

pub fn parameter(input: &str) -> IResult<&str, Parameter> {
    let (rest, (pname, _, pvalue)) = tuple((
            token, 
            tag("="), 
            alt((quoted_string, into(token)))
        ))(input)?;
    
    let param = match pname.to_lowercase().as_ref() {
        "charset" => Parameter::Charset(EmailCharset::from(Cow::Owned(pvalue))),
        "boundary" => Parameter::Boundary(pvalue),
        _ => Parameter::Other(pname, pvalue),
    };

    Ok((rest, param))
}

pub fn content_type(input: &str) -> IResult<&str, Type> {
    let (rest, (ctype, _, csub, params)) = tuple((
            token, tag("/"), token, 
            many0(preceded(tag(";"), parameter))
    ))(input)?;

    let parsed = match ctype.to_lowercase().as_ref() {
        "multipart" => {
            let (boundary_param, unknown_parameters): (Vec<Parameter>, Vec<Parameter>) = params
                .into_iter()
                .partition(|p| matches!(p, Parameter::Boundary(_)));

            // @FIXME: if multiple boundary value is set, only the 
            // first one is picked. We should check that it makes
            // sense with other implementation.
            match boundary_param.into_iter().next() {
                // @FIXME boundary is mandatory. If it is missing,
                // fallback to text/plain. Must check that this behavior
                // is standard...
                None => Type::Text(TextDesc {
                    charset: None,
                    subtype: TextSubtype::Plain, 
                    unknown_parameters
                }),
                Some(Parameter::Boundary(v)) => Type::Multipart(MultipartDesc {
                    subtype: MultipartSubtype::from(csub),
                    unknown_parameters,
                    boundary: v.into(),
                }),
                Some(_) => unreachable!(), // checked above
            }
        },

        "message" => {
            Type::Message(MessageDesc {
                subtype: MessageSubtype::from(csub),
                unknown_parameters: params,
            })
        },

        "text" => {
            let (charset_param, unknown_parameters): (Vec<Parameter>, Vec<Parameter>) = params
                .into_iter()
                .partition(|p| matches!(p, Parameter::Charset(_)));

            let charset = match charset_param.into_iter().next() {
                Some(Parameter::Charset(emlchar)) => Some(emlchar),
                _ => None,
            };

            Type::Text(TextDesc {
                subtype: TextSubtype::from(csub),
                charset: charset,
                unknown_parameters,
            })
        },

        "image" => Type::Image(csub, params),
        "audio" => Type::Audio(csub, params),
        "video" => Type::Video(csub, params),
        "application" => Type::Application(csub, params),
        _ => Type::Other(ctype, csub, params),
    };

    Ok((rest, parsed))
}

pub fn mechanism(input: &str) -> IResult<&str, Mechanism> {
    use Mechanism::*;

    let (input, mecha) = token(input)?;
    let parsed = match mecha.to_lowercase().as_ref() {
        "7bit" => _7Bit,
        "8bit" => _8Bit,
        "binary" => Binary,
        "quoted-printable" => QuotedPrintable,
        "base64" => Base64,
        _ => Other(mecha),
    };

    Ok((input, parsed))
}

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

    #[test]
    fn test_mechanism() {
        assert_eq!(
            Mechanism::try_from(&lazy::Mechanism("7bit")),
            Ok(Mechanism::_7Bit),
        );

        assert_eq!(
            Mechanism::try_from(&lazy::Mechanism("(youhou) 8bit")),
            Ok(Mechanism::_8Bit),
        );
        
        assert_eq!(
            Mechanism::try_from(&lazy::Mechanism("(blip) bInArY (blip blip)")),
            Ok(Mechanism::Binary),
        );

        assert_eq!(
            Mechanism::try_from(&lazy::Mechanism(" base64 ")),
            Ok(Mechanism::Base64),
        );

        assert_eq!(
            Mechanism::try_from(&lazy::Mechanism(" Quoted-Printable ")),
            Ok(Mechanism::QuotedPrintable),
        );
    }
}
