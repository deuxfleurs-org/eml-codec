use crate::error::IMFError;
use crate::multipass::guess_charset;
use crate::fragments::whitespace::headers;

#[derive(Debug, PartialEq)]
pub struct Parsed<'a> {
    pub header: &'a [u8],
    pub body: &'a [u8],
}

pub fn new<'a>(buffer: &'a [u8]) -> Result<Parsed<'a>, IMFError<'a>> {
    headers(buffer)
        .map_err(|e| IMFError::Segment(e))
        .map(|(body, header)| Parsed { header, body })
}

impl<'a> Parsed<'a> {
    pub fn charset(&'a self) -> guess_charset::Parsed<'a> {
        guess_charset::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_segment() {
        assert_eq!(
            new(&b"From: hello@world.com\r\nDate: 12 Mar 1997 07:33:25 Z\r\n\r\nHello world!"[..]),
            Ok(Parsed {
                header: b"From: hello@world.com\r\nDate: 12 Mar 1997 07:33:25 Z\r\n",
                body: b"Hello world!",
            })
        );
    }
}
