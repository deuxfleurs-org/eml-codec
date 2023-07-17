use nom::{
    IResult,
    bytes::complete::{is_not, tag},
    multi::many0,
    sequence::{pair, tuple},
    combinator::{not, opt, recognize},
};

use crate::fragments::mime::{Mechanism, Type};
use crate::fragments::model::MessageId;
use crate::fragments::misc_token::Unstructured;
use crate::fragments::whitespace::{CRLF, obs_crlf};

#[derive(Debug, PartialEq, Default)]
pub struct PartHeader<'a> {
    pub content_type: Option<&'a Type<'a>>,
    pub content_transfer_encoding: Option<&'a Mechanism<'a>>,
    pub content_id: Option<&'a MessageId<'a>>,
    pub content_description: Option<&'a Unstructured>,
}

#[derive(Debug, PartialEq)]
pub enum PartNode<'a> {
    Discrete(PartHeader<'a>, &'a [u8]),
    Composite(PartHeader<'a>, Vec<PartNode<'a>>),
}

#[derive(Debug, PartialEq)]
pub enum Delimiter {
    Next,
    Last
}

pub fn boundary<'a>(boundary: &'a [u8]) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], Delimiter> {
    move |input: &[u8]| {
        let (rest, (_, _, _, last, _)) = tuple((obs_crlf, tag(b"--"), tag(boundary), opt(tag(b"--")), obs_crlf))(input)?;
        match last {
            Some(_) => Ok((rest, Delimiter::Last)),
            None => Ok((rest, Delimiter::Next)),
        }
    }
}

pub fn part(input: &[u8]) -> IResult<&[u8], (PartNode, Delimiter)> {
    todo!();
    // parse headers up to CRLF
    // parse body up to boundary
    // returns (PartNode + Delimiter)
}

pub fn preamble<'a>(bound: &'a [u8]) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], &'a [u8]> {
    move |input: &[u8]| {
        recognize(many0(tuple((
            is_not(CRLF), 
            many0(pair(not(boundary(bound)), obs_crlf)),
        ))))(input)
    }
}


pub fn multipart<'a>(bound: &'a [u8]) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], Vec<PartNode<'a>>> {
    move |input: &[u8]| {
        
        todo!();

    }
    // skip to boundary
    // if boundary last stop
    // do
    // --parse part (return PartNode + Delimiter)
    // while boundary not last
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boundary_next() {
        assert_eq!(
            boundary(b"hello")(b"\r\n--hello\r\n"),
            Ok((&b""[..], Delimiter::Next))
        );
    }

    #[test]
    fn test_boundary_last() {
        assert_eq!(
            boundary(b"hello")(b"\r\n--hello--\r\n"),
            Ok((&b""[..], Delimiter::Last))
        );
    }

    #[test]
    fn test_preamble() {
        assert_eq!(
            preamble(b"hello")(b"blip
bloup

blip
bloup--
--bim
--bim--

--hello
Field: Body
"),
            Ok((
                &b"\n--hello\nField: Body\n"[..],
                &b"blip\nbloup\n\nblip\nbloup--\n--bim\n--bim--\n"[..],
            ))
        );
    }
}
