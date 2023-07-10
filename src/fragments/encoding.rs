use std::borrow::Cow;
use nom::{
    IResult,
    branch::alt,
    bytes::complete::{tag, take, take_while1},
    character::complete::{hex_digit1, one_of},
    combinator::map,
    sequence::{preceded, tuple},
    multi::many0,
};

use crate::fragments::mime;

pub fn encoded_word(input: &str) -> IResult<&str, Cow<str>> {
    let (rest, (_, charset, _, enc, _, txt, _)) = tuple((
        tag("=?"), mime::token, tag("?"), one_of("QBqb"), tag("?"), encoded_text, tag("?=")
    ))(input)?;

    match enc {
        // quoted printable
        'q'|'Q' => todo!(),
        // base64
        'b'|'B' => todo!(),
        _ => unreachable!(),
    }
}

fn encoded_text(input: &str) -> IResult<&str, &str> {
    take_while1(is_encoded_text)(input)
}

fn is_encoded_text(c: char) -> bool {
    c.is_ascii() && !c.is_ascii_control() && !c.is_ascii_whitespace()
}

#[derive(PartialEq,Debug)]
pub enum QuotedChunk<'a> {
    Safe(&'a str),
    Encoded(u8),
}

//quoted_printable
pub fn ptext(input: &str) -> IResult<&str, Vec<QuotedChunk>> {
    many0(alt((safe_char, hex_octet)))(input)
}

fn safe_char(input: &str) -> IResult<&str, QuotedChunk> {
  map(take_while1(is_safe_char), |v| QuotedChunk::Safe(v))(input)  
}

fn is_safe_char(c: char) -> bool {
    (c >= '\x21' && c <= '\x3c') ||
        (c >= '\x3e' && c <= '\x7e')
}

fn hex_octet(input: &str) -> IResult<&str, QuotedChunk> {
    use nom;
    use nom::error::*;

    let (rest, hstr) = preceded(tag("="), take(2usize))(input)?;

    let parsed = u8::from_str_radix(hstr, 16)
        .map_err(|_| nom::Err::Error(Error::new(input, ErrorKind::Verify)))?;

    Ok((rest, QuotedChunk::Encoded(parsed)))
}

//base64 (maybe use a crate)


#[cfg(test)]
mod tests {
    use super::*;

    // =?iso8859-1?Q?Accus=E9_de_r=E9ception_(affich=E9)?=
    #[test]
    fn test_ptext() {
        assert_eq!(
            ptext("Accus=E9_de_r=E9ception_(affich=E9)"),
            Ok(("", vec![
                QuotedChunk::Safe("Accus"),
                QuotedChunk::Encoded(0xe9),
                QuotedChunk::Safe("_de_r"),
                QuotedChunk::Encoded(0xe9),
                QuotedChunk::Safe("ception_(affich"),
                QuotedChunk::Encoded(0xe9),
                QuotedChunk::Safe(")"),
            ]))
        );
    }
}
