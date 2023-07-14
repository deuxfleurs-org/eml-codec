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
pub struct Version {
    pub major: u32,
    pub minor: u32,
}

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

/// Specific implementation of charset
///
/// imf_codec has its own charset list to follow IANA's one.
/// encoding_rs implements a different standard that does not know US_ASCII.
/// using encoding_rs datastructures directly would lead to a loss of information.
/// https://www.iana.org/assignments/character-sets/character-sets.xhtml
#[allow(non_camel_case_types)]
#[derive(Debug, PartialEq)]
pub enum EmailCharset<'a> {
    US_ASCII,
    ISO_8859_1,
    ISO_8859_2,
    ISO_8859_3,
    ISO_8859_4,
    ISO_8859_5,
    ISO_8859_6,
    ISO_8859_7,
    ISO_8859_8,
    ISO_8859_9,
    ISO_8859_10,
    Shift_JIS,
    EUC_JP,
    ISO_2022_KR,
    EUC_KR,
    ISO_2022_JP,
    ISO_2022_JP_2,
    ISO_8859_6_E,
    ISO_8859_6_I,
    ISO_8859_8_E,
    ISO_8859_8_I,
    GB2312,
    Big5,
    KOI8_R,
    UTF_8,
    Other(Cow<'a, str>),
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
impl<'a> From<&'a str> for EmailCharset<'a> {
    fn from(s: &'a str) -> Self {
        EmailCharset::from(Cow::Borrowed(s))
    }
}

impl<'a> From<Cow<'a, str>> for EmailCharset<'a> {
    fn from(s: Cow<'a, str>) -> Self {
        match s.to_lowercase().as_ref() {
            "us-ascii" | "ascii" => EmailCharset::US_ASCII,
            "iso-8859-1" => EmailCharset::ISO_8859_1,
            "iso-8859-2" => EmailCharset::ISO_8859_2,
            "iso-8859-3" => EmailCharset::ISO_8859_3,
            "iso-8859-4" => EmailCharset::ISO_8859_4,
            "iso-8859-5" => EmailCharset::ISO_8859_5,
            "iso-8859-6" => EmailCharset::ISO_8859_6,
            "iso-8859-7" => EmailCharset::ISO_8859_7,
            "iso-8859-8" => EmailCharset::ISO_8859_8,
            "iso-8859-9" => EmailCharset::ISO_8859_9,
            "iso-8859-10" => EmailCharset::ISO_8859_10,
            "shift_jis" => EmailCharset::Shift_JIS,
            "euc-jp" => EmailCharset::EUC_JP,
            "iso-2022-kr" => EmailCharset::ISO_2022_KR,
            "euc-kr" => EmailCharset::EUC_KR,
            "iso-2022-jp" => EmailCharset::ISO_2022_JP,
            "iso-2022-jp-2" => EmailCharset::ISO_2022_JP_2,
            "iso-8859-6-e" => EmailCharset::ISO_8859_6_E,
            "iso-8859-6-i" => EmailCharset::ISO_8859_6_I,
            "iso-8859-8-e" => EmailCharset::ISO_8859_8_E,
            "iso-8859-8-i" => EmailCharset::ISO_8859_8_I,
            "gb2312" => EmailCharset::GB2312,
            "big5" => EmailCharset::Big5,
            "koi8-r" => EmailCharset::KOI8_R,
            "utf-8" | "utf8" => EmailCharset::UTF_8,
            _ => EmailCharset::Other(s)
        }
    }   
}

impl<'a> EmailCharset<'a> {
    pub fn as_str(&'a self) -> &'a str {
        use EmailCharset::*;
        match self {
            US_ASCII => "US-ASCII",
            ISO_8859_1 => "ISO-8859-1",
            ISO_8859_2 => "ISO-8859-2",
            ISO_8859_3 => "ISO-8859-3",
            ISO_8859_4 => "ISO-8859-4",
            ISO_8859_5 => "ISO-8859-5",
            ISO_8859_6 => "ISO-8859-6",
            ISO_8859_7 => "ISO-8859-7",
            ISO_8859_8 => "ISO-8859-8",
            ISO_8859_9 => "ISO-8859-9",
            ISO_8859_10 => "ISO-8859-10",
            Shift_JIS => "Shift_JIS",
            EUC_JP => "EUC-JP",
            ISO_2022_KR => "ISO-2022-KR",
            EUC_KR => "EUC-KR",
            ISO_2022_JP => "ISO-2022-JP",
            ISO_2022_JP_2 => "ISO-2022-JP-2",
            ISO_8859_6_E => "ISO-8859-6-E",
            ISO_8859_6_I => "ISO-8859-6-I",
            ISO_8859_8_E => "ISO-8859-8-E",
            ISO_8859_8_I => "ISO-8859-8-I",
            GB2312 => "GB2312",
            Big5 => "Big5",
            KOI8_R => "KOI8-R",
            UTF_8 => "UTF-8",
            Other(raw) => raw.as_ref(),
        }
    }

    pub fn as_encoding(&self) -> &'static Encoding {
        Encoding::for_label(self.as_str().as_bytes())
            .unwrap_or(encoding_rs::WINDOWS_1252)
    }
}

impl<'a> TryFrom<&'a lazy::Version<'a>> for Version {
    type Error = IMFError<'a>;

    fn try_from(vs: &'a lazy::Version<'a>) -> Result<Self, Self::Error> {
        version(vs.0)
            .map(|(_, v)| v)
            .map_err(|e| IMFError::Version(e))
    }
}

impl<'a> TryFrom<&'a lazy::Type<'a>> for Type<'a> {
    type Error = IMFError<'a>;

    fn try_from(tp: &'a lazy::Type<'a>) -> Result<Self, Self::Error> {
        content_type(tp.0)
            .map(|(_, v)| v)
            .map_err(|e| IMFError::ContentType(e))
    }
}

impl<'a> TryFrom<&'a lazy::Mechanism<'a>> for Mechanism<'a> {
    type Error = IMFError<'a>;

    fn try_from(mc: &'a lazy::Mechanism<'a>) -> Result<Self, Self::Error> {
        mechanism(mc.0)
            .map(|(_, v)| v)
            .map_err(|e| IMFError::Mechanism(e))
    }
}

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


pub fn version(input: &str) -> IResult<&str, Version> {
    let (rest, (_, major, _, _, _, minor, _)) = tuple((
        opt(cfws),
        character::u32,
        opt(cfws),
        tag("."),
        opt(cfws),
        character::u32,
        opt(cfws),
    ))(input)?;
    Ok((rest, Version { major, minor }))
}

/// Token allowed characters
fn is_token_text(c: char) -> bool {
    c.is_ascii() && !c.is_ascii_control() && !c.is_ascii_whitespace() && !"()<>@,;:\\\"/[]?=".contains(c)
}

/// Token
///
/// `[CFWS] 1*token_text [CFWS]`
pub fn token(input: &str) -> IResult<&str, &str> {
    delimited(opt(cfws), take_while1(is_token_text), opt(cfws))(input)
}

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
    fn test_charset() {
        assert_eq!(
            EmailCharset::from("Us-Ascii").as_str(),
            "US-ASCII",
        );

        assert_eq!(
            EmailCharset::from("Us-Ascii").as_encoding(),
            encoding_rs::WINDOWS_1252,
        );

        assert_eq!(
            EmailCharset::from("ISO-8859-1").as_encoding(),
            encoding_rs::WINDOWS_1252,
        );

        assert_eq!(
            EmailCharset::from("utf-8").as_encoding(),
            encoding_rs::UTF_8,
        );

        assert_eq!(
            EmailCharset::from("utf8").as_encoding(),
            encoding_rs::UTF_8,
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
