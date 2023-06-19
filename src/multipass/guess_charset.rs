use std::borrow::Cow;
use chardetng::EncodingDetector;
use encoding_rs::Encoding;

use crate::multipass::segment::Segment;

#[derive(Debug, PartialEq)]
pub struct GuessCharset<'a> {
    pub header: Cow<'a, str>,
    pub encoding: &'static Encoding,
    pub malformed: bool,
    pub body: &'a [u8],
}

const IS_LAST_BUFFER: bool = true;
const ALLOW_UTF8: bool = true;
const NO_TLD: Option<&[u8]> = None;

impl<'a> From<&'a Segment<'a>> for GuessCharset<'a> {
    fn from(seg: &'a Segment<'a>) -> Self {
        // Create detector
        let mut detector = EncodingDetector::new();
        detector.feed(&seg.header, IS_LAST_BUFFER);

        // Get encoding
        let enc: &Encoding = detector.guess(NO_TLD, ALLOW_UTF8);
        let (header, encoding, malformed) = enc.decode(&seg.header);

        GuessCharset { header, encoding, malformed, body: seg.body }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_charset() {
        assert_eq!(
            GuessCharset::from(&Segment {
                body: b"Hello world!", 
                header: b"From: hello@world.com\r\nDate: 12 Mar 1997 07:33:25 Z\r\n",
            }),
            GuessCharset {
                header: "From: hello@world.com\r\nDate: 12 Mar 1997 07:33:25 Z\r\n".into(),
                encoding: encoding_rs::UTF_8,
                malformed: false,
                body: b"Hello world!",
            });
    }
}
