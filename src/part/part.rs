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
    Multipart(Multipart<MIME>, Vec<Part<'a>>),
    Message(MIME<Message>, Message, Part<'a>),
    Text(MIME<Text>, &'a [u8]),
    Binary(MIME<Binary>, &'a [u8]),
}

pub struct Part<'a> {
    List(Vec<Part<'a>>),
    Single(Part<'a>),
    Leaf(&'a [u8]),
}

pub fn message() -> IResult<&[u8], Part> {
}

pub fn multipart<'a>(m: Multipart<MIME>) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], Part<'a>> {
    move |input: &[u8]| {
        let (mut input_loop, _) = preamble(m.ctype.boundary)(input)?;
        let mut parts: Vec<Part> = vec![];
        loop {
            let input = match boundary(m.ctype.boundary)(input_loop) {
                Err(_) => return Ok((input_loop, parts)),
                Ok((inp, Delimiter::Last)) => return Ok((inp, Part::List(parts))),
                Ok((inp, Delimiter::Next)) => inp,
            };

            // parse mime headers
            let (input, fields) = header_in_boundaries(ctype.boundary, content)(input)?;
            let mime = fields.to_mime();

            // parse mime body
            match mime.part_type {
                Type::Multipart(m) => multipart(m),
                Type::Message(m) => message(m),
                Type::Text(t) | Type::Binary
            }

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
