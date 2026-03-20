#[cfg(feature = "arbitrary")]
use arbitrary::Arbitrary;
use bounded_static::ToStatic;
use encoding_rs::Encoding;
use crate::text::words::is_vchar;
#[cfg(feature = "arbitrary")]
use crate::fuzz_eq::FuzzEq;

/// Specific implementation of charset
///
/// imf_codec has its own charset list to follow IANA's one.
/// encoding_rs implements a different standard that does not know US_ASCII.
/// using encoding_rs datastructures directly would lead to a loss of information.
/// <https://www.iana.org/assignments/character-sets/character-sets.xhtml>
#[allow(non_camel_case_types)]
#[derive(Debug, PartialEq, Default, Clone, ToStatic)]
pub enum EmailCharset {
    #[default]
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
    // Must contain only printable characters (text::words::is_vchar).
    // Must be nonempty, and must not represent any of the known charsets.
    Unknown(Vec<u8>),
}

impl<T: AsRef<[u8]>> From<T> for EmailCharset {
    fn from(bytes: T) -> Self {
        let s = bytes.as_ref().to_ascii_lowercase();
        match s.as_slice() {
            b"us-ascii" | b"ascii" => EmailCharset::US_ASCII,
            b"iso-8859-1" => EmailCharset::ISO_8859_1,
            b"iso-8859-2" => EmailCharset::ISO_8859_2,
            b"iso-8859-3" => EmailCharset::ISO_8859_3,
            b"iso-8859-4" => EmailCharset::ISO_8859_4,
            b"iso-8859-5" => EmailCharset::ISO_8859_5,
            b"iso-8859-6" => EmailCharset::ISO_8859_6,
            b"iso-8859-7" => EmailCharset::ISO_8859_7,
            b"iso-8859-8" => EmailCharset::ISO_8859_8,
            b"iso-8859-9" => EmailCharset::ISO_8859_9,
            b"iso-8859-10" => EmailCharset::ISO_8859_10,
            b"shift_jis" => EmailCharset::Shift_JIS,
            b"euc-jp" => EmailCharset::EUC_JP,
            b"iso-2022-kr" => EmailCharset::ISO_2022_KR,
            b"euc-kr" => EmailCharset::EUC_KR,
            b"iso-2022-jp" => EmailCharset::ISO_2022_JP,
            b"iso-2022-jp-2" => EmailCharset::ISO_2022_JP_2,
            b"iso-8859-6-e" => EmailCharset::ISO_8859_6_E,
            b"iso-8859-6-i" => EmailCharset::ISO_8859_6_I,
            b"iso-8859-8-e" => EmailCharset::ISO_8859_8_E,
            b"iso-8859-8-i" => EmailCharset::ISO_8859_8_I,
            b"gb2312" => EmailCharset::GB2312,
            b"big5" => EmailCharset::Big5,
            b"koi8-r" => EmailCharset::KOI8_R,
            b"utf-8" | b"utf8" => EmailCharset::UTF_8,
            _ => {
                // Filter out bytes that are not printable, in case there are some…
                let sanitized = bytes.as_ref().iter().cloned().filter(|b| is_vchar(*b));
                EmailCharset::Unknown(sanitized.collect())
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
        use EmailCharset::*;
        match self {
            US_ASCII => b"US-ASCII",
            ISO_8859_1 => b"ISO-8859-1",
            ISO_8859_2 => b"ISO-8859-2",
            ISO_8859_3 => b"ISO-8859-3",
            ISO_8859_4 => b"ISO-8859-4",
            ISO_8859_5 => b"ISO-8859-5",
            ISO_8859_6 => b"ISO-8859-6",
            ISO_8859_7 => b"ISO-8859-7",
            ISO_8859_8 => b"ISO-8859-8",
            ISO_8859_9 => b"ISO-8859-9",
            ISO_8859_10 => b"ISO-8859-10",
            Shift_JIS => b"Shift_JIS",
            EUC_JP => b"EUC-JP",
            ISO_2022_KR => b"ISO-2022-KR",
            EUC_KR => b"EUC-KR",
            ISO_2022_JP => b"ISO-2022-JP",
            ISO_2022_JP_2 => b"ISO-2022-JP-2",
            ISO_8859_6_E => b"ISO-8859-6-E",
            ISO_8859_6_I => b"ISO-8859-6-I",
            ISO_8859_8_E => b"ISO-8859-8-E",
            ISO_8859_8_I => b"ISO-8859-8-I",
            GB2312 => b"GB2312",
            Big5 => b"Big5",
            KOI8_R => b"KOI8-R",
            UTF_8 => b"UTF-8",
            Unknown(s) => &s,
        }
    }

    pub fn as_encoding(&self) -> &'static Encoding {
        // XXX currently the `Unknown` case is passed through as-is to encoding_rs,
        // is this what we should do?
        Encoding::for_label(self.as_bytes()).unwrap_or(encoding_rs::WINDOWS_1252)
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for EmailCharset {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(
            match u.int_in_range(0..=25)? {
                0 => EmailCharset::US_ASCII,
                1 => EmailCharset::ISO_8859_1,
                2 => EmailCharset::ISO_8859_2,
                3 => EmailCharset::ISO_8859_3,
                4 => EmailCharset::ISO_8859_4,
                5 => EmailCharset::ISO_8859_5,
                6 => EmailCharset::ISO_8859_6,
                7 => EmailCharset::ISO_8859_7,
                8 => EmailCharset::ISO_8859_8,
                9 => EmailCharset::ISO_8859_9,
                10 => EmailCharset::ISO_8859_10,
                11 => EmailCharset::Shift_JIS,
                12 => EmailCharset::EUC_JP,
                13 => EmailCharset::ISO_2022_KR,
                14 => EmailCharset::EUC_KR,
                15 => EmailCharset::ISO_2022_JP,
                16 => EmailCharset::ISO_2022_JP_2,
                17 => EmailCharset::ISO_8859_6_E,
                18 => EmailCharset::ISO_8859_6_I,
                19 => EmailCharset::ISO_8859_8_E,
                20 => EmailCharset::ISO_8859_8_I,
                21 => EmailCharset::GB2312,
                22 => EmailCharset::Big5,
                23 => EmailCharset::KOI8_R,
                24 => EmailCharset::UTF_8,
                25 => {
                    // don't bother generating unknown charsets, use a dummy
                    EmailCharset::Unknown(b"unk".to_vec())
                }
                _ => unreachable!(),
            }
        )
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
        assert_eq!(EmailCharset::from(&b"Us-Ascii"[..]).as_bytes(), b"US-ASCII",);

        assert_eq!(
            EmailCharset::from(&b"Us-Ascii"[..]).as_encoding(),
            encoding_rs::WINDOWS_1252,
        );

        assert_eq!(
            EmailCharset::from(&b"ISO-8859-1"[..]).as_encoding(),
            encoding_rs::WINDOWS_1252,
        );

        assert_eq!(
            EmailCharset::from(&b"utf-8"[..]).as_encoding(),
            encoding_rs::UTF_8,
        );

        assert_eq!(
            EmailCharset::from(&b"utf8"[..]).as_encoding(),
            encoding_rs::UTF_8,
        );

        assert_eq!(
            EmailCharset::from(&b"!*\x00\x01abc"[..]),
            EmailCharset::Unknown(b"!*abc".to_vec()),
        );
    }
}
