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
use encoding_rs::Encoding;

use crate::fragments::mime;

pub fn encoded_word(input: &str) -> IResult<&str, String> {
    let (rest, (_, charset, _, enc, _, txt, _)) = tuple((
        tag("=?"), mime::token, tag("?"), one_of("QBqb"), tag("?"), ptext, tag("?=")
    ))(input)?;

    let renc =  Encoding::for_label(charset.as_bytes()).unwrap_or(encoding_rs::WINDOWS_1252);

    let parsed = match enc {
        // quoted printable
        'q'|'Q' => decode_quoted_encoding(renc, txt.iter()),
        // base64
        'b'|'B' => todo!(),
        _ => unreachable!(),
    };

    Ok((rest, parsed))
}

fn decode_quoted_encoding<'a>(enc: &'static Encoding, q: impl Iterator<Item = &'a QuotedChunk<'a>>) -> String {
    q.fold(
        String::new(), 
        |mut acc, c| {
            let dec = match c {
                QuotedChunk::Safe(v) => Cow::Borrowed(*v),
                QuotedChunk::Space => Cow::Borrowed(" "),
                QuotedChunk::Encoded(v) => {
                    let w = &[*v];
                    let (d, _, _) = enc.decode(w);
                    Cow::Owned(d.into_owned())
                },
            };
            acc.push_str(dec.as_ref());
            acc
        })
}


#[derive(PartialEq,Debug)]
pub enum QuotedChunk<'a> {
    Safe(&'a str),
    Encoded(u8),
    Space,
}

//quoted_printable
pub fn ptext(input: &str) -> IResult<&str, Vec<QuotedChunk>> {
    many0(alt((safe_char2, encoded_space, hex_octet)))(input)
}

fn safe_char2(input: &str) -> IResult<&str, QuotedChunk> {
  map(take_while1(is_safe_char2), |v| QuotedChunk::Safe(v))(input)  
}


/// RFC2047 section 4.2
/// 8-bit values which correspond to printable ASCII characters other
/// than "=", "?", and "_" (underscore), MAY be represented as those
/// characters.
fn is_safe_char2(c: char) -> bool {
    c.is_ascii() && !c.is_ascii_control() && c != '_' && c != '?' && c != '='
}

fn is_safe_char(c: char) -> bool {
    (c >= '\x21' && c <= '\x3c') ||
        (c >= '\x3e' && c <= '\x7e')
}

fn encoded_space(input: &str) -> IResult<&str, QuotedChunk> {
    map(tag("_"), |_| QuotedChunk::Space)(input)
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
                QuotedChunk::Space,
                QuotedChunk::Safe("de"),
                QuotedChunk::Space,
                QuotedChunk::Safe("r"),
                QuotedChunk::Encoded(0xe9),
                QuotedChunk::Safe("ception"),
                QuotedChunk::Space,
                QuotedChunk::Safe("(affich"),
                QuotedChunk::Encoded(0xe9),
                QuotedChunk::Safe(")"),
            ]))
        );
    }


    #[test]
    fn test_decode_word() {
        assert_eq!(
            encoded_word("=?iso8859-1?Q?Accus=E9_de_r=E9ception_(affich=E9)?="),
            Ok(("", "Accusé de réception (affiché)".into())),
        );
    }
}
