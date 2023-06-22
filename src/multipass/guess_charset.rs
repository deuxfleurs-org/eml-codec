use chardetng::EncodingDetector;
use encoding_rs::Encoding;
use std::borrow::Cow;

use crate::error::IMFError;
use crate::multipass::extract_fields;
use crate::multipass::segment;

#[derive(Debug, PartialEq)]
pub struct Parsed<'a> {
    pub header: Cow<'a, str>,
    pub encoding: &'static Encoding,
    pub malformed: bool,
    pub body: &'a [u8],
}

const IS_LAST_BUFFER: bool = true;
const ALLOW_UTF8: bool = true;
const NO_TLD: Option<&[u8]> = None;

pub fn new<'a>(seg: &'a segment::Parsed<'a>) -> Parsed<'a> {
    // Create detector
    let mut detector = EncodingDetector::new();
    detector.feed(&seg.header, IS_LAST_BUFFER);

    // Get encoding
    let enc: &Encoding = detector.guess(NO_TLD, ALLOW_UTF8);
    let (header, encoding, malformed) = enc.decode(&seg.header);
    Parsed {
        header,
        encoding,
        malformed,
        body: seg.body,
    }
}

impl<'a> Parsed<'a> {
    pub fn fields(&'a self) -> Result<extract_fields::Parsed<'a>, IMFError<'a>> {
        extract_fields::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_charset() {
        assert_eq!(
            new(&segment::Parsed {
                body: b"Hello world!",
                header: b"From: hello@world.com\r\nDate: 12 Mar 1997 07:33:25 Z\r\n",
            }),
            Parsed {
                header: "From: hello@world.com\r\nDate: 12 Mar 1997 07:33:25 Z\r\n".into(),
                encoding: encoding_rs::UTF_8,
                malformed: false,
                body: b"Hello world!",
            }
        );
    }
}
