use std::borrow::Cow;

use crate::error::IMFError;
use crate::fragments::encoding;
use crate::multipass::extract_fields;
use crate::multipass::segment;

#[derive(Debug, PartialEq)]
pub struct Parsed<'a> {
    pub header: Cow<'a, str>,
    pub body: &'a [u8],
}

pub fn new<'a>(seg: &'a segment::Parsed<'a>) -> Parsed<'a> {
    Parsed {
        header: encoding::header_decode(&seg.header),
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
