use crate::i18n::ContainsUtf8;
use crate::text::ascii;
use crate::text::words::{is_mime_atom_text, MIMEAtom};
use bounded_static::{IntoBoundedStatic, ToBoundedStatic};
use charset::Charset;
use nom::{bytes::complete::take_while1, combinator::map_res, IResult};
#[cfg(feature = "tracing-recover")]
use tracing::warn;
#[cfg(feature = "arbitrary")]
use {crate::fuzz_eq::FuzzEq, arbitrary::Arbitrary, eyre::eyre};

/// Email charsets are defined by IANA
/// <https://www.iana.org/assignments/character-sets/character-sets.xhtml>
///
/// We piggy-back on the "charset" library that is specifically designed for
/// email.
#[allow(non_camel_case_types)]
#[derive(Clone, ContainsUtf8, Debug, Default, PartialEq)]
#[contains_utf8(false)]
pub enum EmailCharset {
    #[default]
    US_ASCII,
    Charset(Charset),
    // Must be non-empty and contain characters satisfying `is_charset_text`
    Unknown(String),
}

impl TryFrom<&[u8]> for EmailCharset {
    type Error = ();

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        match bytes.to_ascii_lowercase().as_slice() {
            b"us-ascii" | b"ascii" => Ok(Self::US_ASCII),
            _ => {
                if let Some(c) = Charset::for_label(bytes) {
                    Ok(Self::Charset(c))
                } else if !bytes.is_empty() && bytes.iter().all(|c| is_charset_text(*c)) {
                    let c = String::from_utf8_lossy(bytes).to_string();
                    #[cfg(feature = "tracing-recover")]
                    warn!(value = c, "unknown charset");
                    Ok(Self::Unknown(c))
                } else {
                    Err(())
                }
            }
        }
    }
}

impl ToString for EmailCharset {
    fn to_string(&self) -> String {
        String::from_utf8_lossy(self.as_bytes()).into()
    }
}

impl EmailCharset {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::US_ASCII => b"us-ascii",
            Self::Charset(c) => c.name().as_bytes(),
            Self::Unknown(s) => s.as_bytes(),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::US_ASCII => "us-ascii",
            Self::Charset(c) => c.name(),
            Self::Unknown(s) => s.as_str(),
        }
    }

    /// A charset is also a valid `MIMEAtom`
    pub fn to_mime_atom(&self) -> MIMEAtom<'static> {
        MIMEAtom(self.as_bytes().to_vec().into())
    }

    pub fn utf8() -> Self {
        Self::Charset(Charset::for_encoding(encoding_rs::UTF_8))
    }

    pub fn decode<'a>(&self, bytes: &'a [u8]) -> std::borrow::Cow<'a, str> {
        match self {
            Self::US_ASCII | Self::Unknown(_) => charset::decode_ascii(bytes),
            Self::Charset(c) => {
                let (s, _has_malformed) = c.decode_without_bom_handling(bytes);
                s
            }
        }
    }
}

impl IntoBoundedStatic for EmailCharset {
    type Static = Self;
    fn into_static(self) -> Self::Static {
        self
    }
}

impl ToBoundedStatic for EmailCharset {
    type Static = Self;
    fn to_static(&self) -> Self::Static {
        self.clone()
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for EmailCharset {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        // preselect some charsets to help the fuzzer
        match u.int_in_range(0..=6)? {
            0 => Ok(Self::US_ASCII),
            1 => Ok(Self::utf8()),
            2 => Ok(Self::try_from(&b"KOI-8R"[..]).unwrap()),
            3 => Ok(Self::try_from(&b"iso-8859-1"[..]).unwrap()),
            4 => Ok(Self::try_from(&b"iso-8859-15"[..]).unwrap()),
            5 => Ok(Self::try_from(&b"GBK"[..]).unwrap()),
            6 => {
                let label: &[u8] = u.arbitrary()?;
                Self::try_from(label).map_err(|()| arbitrary::Error::IncorrectFormat)
            }
            _ => unreachable!(),
        }
    }
}
#[cfg(feature = "arbitrary")]
impl FuzzEq for EmailCharset {
    fn fuzz_eq(&self, other: &Self) -> eyre::Result<()> {
        if self != other {
            Err(eyre!("EmailCharset mismatch"))
        } else {
            Ok(())
        }
    }
}

pub fn charset(input: &[u8]) -> IResult<&[u8], EmailCharset> {
    map_res(take_while1(is_charset_text), |word| {
        EmailCharset::try_from(word).map_err(|()| {
            nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Verify))
        })
    })(input)
}

// RFC 2047
//
// charset = token
// token = 1*<Any CHAR except SPACE, CTLs, and especials>
// especials = "(" / ")" / "<" / ">" / "@" / "," / ";" / ":" / "\" /
//             <"> / "/" / "[" / "]" / "?" / "." / "="
//
// NOTE: `token` is the same as our MIMEAtom but with "." additionally excluded!
fn is_charset_text(c: u8) -> bool {
    is_mime_atom_text(c) && c != ascii::PERIOD
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_charset() {
        assert_eq!(
            EmailCharset::try_from(&b"Us-Ascii"[..]).unwrap().as_bytes(),
            b"us-ascii",
        );

        assert_eq!(
            EmailCharset::try_from(&b"Us-Ascii"[..]).unwrap(),
            EmailCharset::US_ASCII,
        );

        assert_eq!(
            EmailCharset::try_from(&b"ISO-8859-1"[..])
                .unwrap()
                .as_bytes(),
            b"windows-1252",
        );

        assert_eq!(
            EmailCharset::try_from(&b"utf-8"[..]).unwrap().as_bytes(),
            b"UTF-8",
        );

        assert_eq!(
            EmailCharset::try_from(&b"utf8"[..]).unwrap().as_bytes(),
            b"UTF-8",
        );

        assert_eq!(
            EmailCharset::try_from(&b"!*abc"[..]),
            Ok(EmailCharset::Unknown("!*abc".to_string())),
        );

        assert_eq!(EmailCharset::try_from(&b""[..]), Err(()),);

        assert_eq!(EmailCharset::try_from(&b"!*\x00\x01abc"[..]), Err(()),);

        assert_eq!(EmailCharset::try_from(&b"a.b"[..]), Err(()),);
    }
}
