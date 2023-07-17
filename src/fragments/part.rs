use nom::{
    IResult,
    branch::alt,
    bytes::complete::{is_not, tag},
    multi::many0,
    sequence::{pair, preceded, tuple},
    combinator::{not, opt, recognize},
};

use crate::fragments::mime::{Mechanism, Type};
use crate::fragments::model::MessageId;
use crate::fragments::misc_token::Unstructured;
use crate::fragments::whitespace::{CRLF, headers, line, obs_crlf};

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
        let (rest, (_, _, _, last, _)) = tuple((obs_crlf, tag(b"--"), tag(boundary), opt(tag(b"--")), opt(obs_crlf)))(input)?;
        match last {
            Some(_) => Ok((rest, Delimiter::Last)),
            None => Ok((rest, Delimiter::Next)),
        }
    }
}

pub fn part<'a>(bound: &'a [u8]) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], &'a [u8]> {
    move |input: &[u8]| {
        recognize(many0(pair(
            not(boundary(bound)),
            alt((is_not(CRLF), obs_crlf)),
        )))(input)
    }
}

pub fn preamble<'a>(bound: &'a [u8]) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], &'a [u8]> {
    move |input: &[u8]| {
        recognize(many0(tuple((
            is_not(CRLF), 
            many0(pair(not(boundary(bound)), obs_crlf)),
        ))))(input)
    }
}

// Returns Ok even if an error is encountered while parsing
// the different mimes.
pub fn multipart<'a>(bound: &'a [u8]) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], Vec<&'a [u8]>> {
    move |input: &[u8]| {
        let (mut input_loop, _) = preamble(bound)(input)?;
        let mut parts: Vec<&[u8]> = vec![];
        loop {
            let input2 = match boundary(bound)(input_loop) {
                Err(_) => return Ok((input_loop, parts)),
                Ok((inp, Delimiter::Last)) => return Ok((inp, parts)),
                Ok((inp, Delimiter::Next)) => inp,
            };

            let input3 = match part(bound)(input2) {
                Err(_) => return Ok((input2, parts)),
                Ok((inp, part)) => {
                    parts.push(part);
                    inp
                }
            };

            input_loop = input3;
        }
    }
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

    #[test]
    fn test_part() {
        assert_eq!(
            part(b"simple boundary")(b"Content-type: text/plain; charset=us-ascii

This is explicitly typed plain US-ASCII text.
It DOES end with a linebreak.

--simple boundary--
"),
            Ok((
                &b"\n--simple boundary--\n"[..], 
                &b"Content-type: text/plain; charset=us-ascii\n\nThis is explicitly typed plain US-ASCII text.\nIt DOES end with a linebreak.\n"[..],
            ))
        );
    }

    #[test]
    fn test_multipart() {
        assert_eq!(
            multipart(b"simple boundary")(b"This is the preamble.  It is to be ignored, though it
is a handy place for composition agents to include an
explanatory note to non-MIME conformant readers.

--simple boundary

This is implicitly typed plain US-ASCII text.
It does NOT end with a linebreak.
--simple boundary
Content-type: text/plain; charset=us-ascii

This is explicitly typed plain US-ASCII text.
It DOES end with a linebreak.

--simple boundary--

This is the epilogue. It is also to be ignored.
"),
            Ok((&b"\nThis is the epilogue. It is also to be ignored.\n"[..],
                vec![
                    &b"\nThis is implicitly typed plain US-ASCII text.\nIt does NOT end with a linebreak."[..],
                    &b"Content-type: text/plain; charset=us-ascii\n\nThis is explicitly typed plain US-ASCII text.\nIt DOES end with a linebreak.\n"[..],
                ]
            )),
        );
    }
}
