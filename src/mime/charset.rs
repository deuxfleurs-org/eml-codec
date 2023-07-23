use encoding_rs::Encoding;

/// Specific implementation of charset
///
/// imf_codec has its own charset list to follow IANA's one.
/// encoding_rs implements a different standard that does not know US_ASCII.
/// using encoding_rs datastructures directly would lead to a loss of information.
/// https://www.iana.org/assignments/character-sets/character-sets.xhtml
#[allow(non_camel_case_types)]
#[derive(Debug, PartialEq, Default, Clone)]
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
    Unknown,
}

impl<'a> From<&'a str> for EmailCharset {
    fn from(s: &'a str) -> Self {
        Self::from(s.as_bytes())
    }
}

impl<'a> From<&'a [u8]> for EmailCharset {
    fn from(s: &'a [u8]) -> Self {
        match s.to_ascii_lowercase().as_slice() {
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
            _ => EmailCharset::Unknown,
        }

    }
}

impl EmailCharset {
    pub fn as_str(&self) -> &'static str {
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
            Unknown => "UTF-8",
        }
    }

    pub fn as_encoding(&self) -> &'static Encoding {
        Encoding::for_label(self.as_str().as_bytes())
            .unwrap_or(encoding_rs::WINDOWS_1252)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_charset() {
        assert_eq!(
            EmailCharset::from(&b"Us-Ascii"[..]).as_str(),
            "US-ASCII",
        );

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
    }
}
