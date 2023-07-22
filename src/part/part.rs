use nom::{
    IResult,
    branch::alt,
    bytes::complete::{is_not, tag},
    multi::many0,
    sequence::{pair, preceded, tuple},
    combinator::{not, opt, recognize},
};

use crate::mime::r#type;

pub struct Part<'a> {
    Multipart(r#type::Multipart, Vec<Part<'a>>),
    Message(r#type::Message, Message, Part<'a>),
    Text(r#type::Text, &'a [u8]),
    Binary(&'a [u8]),
}

pub fn message() -> IResult<&[u8], Part> {
}

pub fn multipart<'a>(ctype: Type) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], Part<'a>> {
    move |input: &[u8]| {
        let (mut input_loop, _) = preamble(ctype.boundary)(input)?;
        let mut parts: Vec<Part> = vec![];
        loop {
            let input = match boundary(ctype.boundary)(input_loop) {
                Err(_) => return Ok((input_loop, parts)),
                Ok((inp, Delimiter::Last)) => return Ok((inp, Part::Multipart(ctype, parts))),
                Ok((inp, Delimiter::Next)) => inp,
            };

            // parse mime headers
            header(content)(input)?;

            // based on headers, parse part

            let input = match part(bound)(input) {
                Err(_) => return Ok((input, parts)),
                Ok((inp, part)) => {
                    parts.push(part);
                    inp
                }
            };

            input_loop = input;
        }
    }

}

pub fn discrete() -> IResult<&[u8], Part> {
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

// FIXME parse email here


// Returns Ok even if an error is encountered while parsing
// the different mimes.
pub fn multipart<'a>(bound: &'a [u8]) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], Vec<&'a [u8]>> {
    move |input: &[u8]| {
        let (mut input_loop, _) = preamble(bound)(input)?;
        let mut parts: Vec<&[u8]> = vec![];
        loop {
            let input = match boundary(bound)(input_loop) {
                Err(_) => return Ok((input_loop, parts)),
                Ok((inp, Delimiter::Last)) => return Ok((inp, parts)),
                Ok((inp, Delimiter::Next)) => inp,
            };

            let input = match part(bound)(input) {
                Err(_) => return Ok((input, parts)),
                Ok((inp, part)) => {
                    parts.push(part);
                    inp
                }
            };

            input_loop = input;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
