use encoding_rs::Encoding;
use nom::{
    bytes::complete::tag, character::complete as character, combinator::opt, sequence::tuple,
    IResult,
};

use crate::error::IMFError;
use crate::fragments::lazy;
use crate::fragments::whitespace::cfws;

#[derive(Debug, PartialEq)]
pub struct Version {
    major: u32,
    minor: u32,
}

#[derive(Debug, PartialEq)]
pub enum Type<'a> {
    // Composite types
    Multipart(MultipartSubtype<'a>),
    Message(MessageSubtype<'a>),

    // Discrete types
    Text(&'a str),
    Image(&'a str),
    Audio(&'a str),
    Video(&'a str),
    Application(&'a str),

    // Unknown
    Other(&'a str, &'a str, Vec<Parameter<'a>>),
}

#[derive(Debug, PartialEq)]
pub enum MultipartSubtype<'a> {
    Alternative(Parameter<'a>),
    Mixed(Parameter<'a>),
    Digest(Parameter<'a>),
    Parallel(Parameter<'a>),
    Other(&'a str, Parameter<'a>),
}

#[derive(Debug, PartialEq)]
pub enum MessageSubtype<'a> {
    RFC822(Vec<Parameter<'a>>),
    Partial(Vec<Parameter<'a>>),
    External(Vec<Parameter<'a>>),
    Other(&'a str, Vec<Parameter<'a>>),
}

#[derive(Debug, PartialEq)]
pub enum Parameter<'a> {
    Charset(EmailCharset<'a>),
    Boundary(&'a str),
    Other(&'a str, &'a str),
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
    Other(&'a str),
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
        match s.to_lowercase().as_ref() {
            "us-ascii" => EmailCharset::US_ASCII,
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
            "utf-8" => EmailCharset::UTF_8,
            _ => EmailCharset::Other(s)
        }
    }   
}

impl<'a> EmailCharset<'a> {
    pub fn as_str(&self) -> &'a str {
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
            Other(raw) => raw,
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
        Ok(Type::Other("", "", vec![]))
    }
}

impl<'a> TryFrom<&'a lazy::Mechanism<'a>> for Mechanism<'a> {
    type Error = IMFError<'a>;

    fn try_from(mc: &'a lazy::Mechanism<'a>) -> Result<Self, Self::Error> {
        Ok(Mechanism::Other(""))
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

#[cfg(test)]
mod tests {
    use super::*;

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

/*    #[test]
    fn test_parameter() {
        assert_eq!(
            parameter("charset=us-ascii (Plain text)"),
            Ok(("", Parameter::charset(EmailCharset::US_ASCII)))
        );

    }
*/
}
