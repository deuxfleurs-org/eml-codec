use nom::{
    branch::alt,
    bytes::complete::{is_not, tag},
    combinator::recognize,
    multi::many0,
    sequence::{pair, terminated},
    IResult,
};

use crate::error::IMFError;
use crate::multipass::guess_charset;

#[derive(Debug, PartialEq)]
pub struct Parsed<'a> {
    pub header: &'a [u8],
    pub body: &'a [u8],
}

const CR: u8 = 0x0D;
const LF: u8 = 0x0A;
const CRLF: &[u8] = &[CR, LF];

pub fn new<'a>(buffer: &'a [u8]) -> Result<Parsed<'a>, IMFError<'a>> {
    terminated(recognize(many0(line)), obs_crlf)(buffer)
        .map_err(|e| IMFError::Segment(e))
        .map(|(body, header)| Parsed { header, body })
}

impl<'a> Parsed<'a> {
    pub fn charset(&'a self) -> guess_charset::Parsed<'a> {
        guess_charset::new(self)
    }
}

fn line(input: &[u8]) -> IResult<&[u8], (&[u8], &[u8])> {
    pair(is_not(CRLF), obs_crlf)(input)
}

fn obs_crlf(input: &[u8]) -> IResult<&[u8], &[u8]> {
    alt((tag(CRLF), tag(&[CR]), tag(&[LF])))(input)
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
