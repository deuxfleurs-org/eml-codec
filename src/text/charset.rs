#[cfg(feature = "arbitrary")]
use {
    arbitrary::Arbitrary,
    crate::fuzz_eq::FuzzEq,
};
#[cfg(feature = "tracing-recover")]
use tracing::warn;
use charset::Charset;
use bounded_static::{IntoBoundedStatic, ToBoundedStatic};
use crate::i18n::ContainsUtf8;
use crate::text::words::is_vchar;

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
    Unknown(String),
}

impl<T: AsRef<[u8]>> From<T> for EmailCharset {
    fn from(bytes: T) -> Self {
        match bytes.as_ref().to_ascii_lowercase().as_slice() {
            b"us-ascii" | b"ascii" => Self::US_ASCII,
            _ => {
                // Filter out bytes that are not ASCII printable, in case there are some…
                let sanitized: String = bytes.as_ref().iter().cloned().filter_map(|b| {
                    (b.is_ascii() && is_vchar(b as char)).then_some(b as char)
                }).collect();
                match Charset::for_label(sanitized.as_bytes()) {
                    Some(c) => Self::Charset(c),
                    None => {
                        #[cfg(feature = "tracing-recover")]
                        warn!(value = sanitized, "unknown charset");
                        EmailCharset::Unknown(sanitized)
                    }
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
            EmailCharset::US_ASCII => b"us-ascii",
            EmailCharset::Charset(c) => c.name().as_bytes(),
            EmailCharset::Unknown(s) => s.as_bytes(),
        }
    }

    pub fn utf8() -> Self {
        Self::Charset(Charset::for_encoding(encoding_rs::UTF_8))
    }

    pub fn decode<'a>(&self, bytes: &'a [u8]) -> std::borrow::Cow<'a, str> {
        match self {
            EmailCharset::US_ASCII | EmailCharset::Unknown(_) =>
                charset::decode_ascii(bytes),
            EmailCharset::Charset(c) => {
                let (s, _has_malformed) = c.decode_without_bom_handling(bytes);
                s
            },
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
            0 => Ok(EmailCharset::US_ASCII),
            1 => Ok(EmailCharset::utf8()),
            2 => Ok(EmailCharset::from(b"KOI-8R")),
            3 => Ok(EmailCharset::from(b"iso-8859-1")),
            4 => Ok(EmailCharset::from(b"iso-8859-15")),
            5 => Ok(EmailCharset::from(b"GBK")),
            6 => {
                let label: &[u8] = u.arbitrary()?;
                Ok(EmailCharset::from(label))
            },
            _ => unreachable!(),
        }
    }
}
#[cfg(feature = "arbitrary")]
impl FuzzEq for EmailCharset {
    fn fuzz_eq(&self, other: &Self) -> bool {
        self == other
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_charset() {
        assert_eq!(
            EmailCharset::from(&b"Us-Ascii"[..]).as_bytes(),
            b"us-ascii",
        );

        assert_eq!(
            EmailCharset::from(&b"Us-Ascii"[..]),
            EmailCharset::US_ASCII,
        );

        assert_eq!(
            EmailCharset::from(&b"ISO-8859-1"[..]).as_bytes(),
            b"windows-1252",
        );

        assert_eq!(
            EmailCharset::from(&b"utf-8"[..]).as_bytes(),
            b"UTF-8",
        );

        assert_eq!(
            EmailCharset::from(&b"utf8"[..]).as_bytes(),
            b"UTF-8",
        );

        assert_eq!(
            EmailCharset::from(&b"!*\x00\x01abc"[..]),
            EmailCharset::Unknown("!*abc".to_string()),
        );
    }
}
