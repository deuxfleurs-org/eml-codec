use std::convert::TryFrom;
use nom::{
    IResult,
    branch::alt,
    bytes::complete::{is_not, tag},
    combinator::recognize,
    sequence::{pair, terminated},
    multi::many0,
};

use crate::error::IMFError;

#[derive(Debug, PartialEq)]
pub struct Segment<'a> {
    pub header: &'a [u8],
    pub body: &'a [u8],
}

const cr: u8 = 0x0D;
const lf: u8 = 0x0A;
const crlf: &[u8] = &[cr, lf];

impl<'a> TryFrom<&'a [u8]> for Segment<'a> {
    type Error = IMFError<'a>;

    fn try_from(buffer: &'a [u8]) -> Result<Self, Self::Error> {
        terminated(
            recognize(many0(line)), 
            obs_crlf
        )(buffer)
            .map_err(|e| IMFError::Segment(e))
            .map(|(body, header)| Segment { header, body })
    }
}

fn line(input: &[u8]) -> IResult<&[u8], (&[u8], &[u8])> {
    pair(
        is_not(crlf), 
        obs_crlf,
    )(input)
}

fn obs_crlf(input: &[u8]) -> IResult<&[u8], &[u8]> {
    alt((tag(crlf), tag(&[cr]), tag(&[lf])))(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_segment() {
        assert_eq!(
            Segment::try_from(&b"From: hello@world.com\r\nDate: 12 Mar 1997 07:33:25 Z\r\n\r\nHello world!"[..]),
            Ok(Segment {
                body: b"Hello world!", 
                header: b"From: hello@world.com\r\nDate: 12 Mar 1997 07:33:25 Z\r\n",
            })
        );
    }
}
