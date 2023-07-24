use encoding_rs::Encoding;

use base64::{engine::general_purpose, Engine as _};
use nom::{
    branch::alt,
    bytes::complete::{tag, take, take_while, take_while1},
    character::complete::one_of,
    character::is_alphanumeric,
    combinator::{map, opt},
    multi::{many0, many1},
    sequence::{preceded, terminated, tuple},
    IResult,
};

use crate::text::ascii;
use crate::text::whitespace::cfws;
use crate::text::words;

pub fn encoded_word(input: &[u8]) -> IResult<&[u8], EncodedWord> {
    alt((encoded_word_quoted, encoded_word_base64))(input)
}

pub fn encoded_word_quoted(input: &[u8]) -> IResult<&[u8], EncodedWord> {
    let (rest, (_, _, charset, _, _, _, txt, _, _)) = tuple((
        opt(cfws),
        tag("=?"),
        words::mime_atom,
        tag("?"),
        one_of("Qq"),
        tag("?"),
        ptext,
        tag("?="),
        opt(cfws),
    ))(input)?;

    let renc = Encoding::for_label(charset).unwrap_or(encoding_rs::WINDOWS_1252);
    let parsed = EncodedWord::Quoted(QuotedWord {
        enc: renc,
        chunks: txt,
    });
    Ok((rest, parsed))
}

pub fn encoded_word_base64(input: &[u8]) -> IResult<&[u8], EncodedWord> {
    let (rest, (_, charset, _, _, _, txt, _)) = tuple((
        tag("=?"),
        words::mime_atom,
        tag("?"),
        one_of("Bb"),
        tag("?"),
        btext,
        tag("?="),
    ))(input)?;

    let renc = Encoding::for_label(charset).unwrap_or(encoding_rs::WINDOWS_1252);
    let parsed = EncodedWord::Base64(Base64Word {
        enc: renc,
        content: txt,
    });
    Ok((rest, parsed))
}

#[derive(PartialEq, Debug, Clone)]
pub enum EncodedWord<'a> {
    Quoted(QuotedWord<'a>),
    Base64(Base64Word<'a>),
}
impl<'a> EncodedWord<'a> {
    pub fn to_string(&self) -> String {
        match self {
            EncodedWord::Quoted(v) => v.to_string(),
            EncodedWord::Base64(v) => v.to_string(),
        }
    }
}

#[derive(PartialEq, Debug, Clone)]
pub struct Base64Word<'a> {
    pub enc: &'static Encoding,
    pub content: &'a [u8],
}

impl<'a> Base64Word<'a> {
    pub fn to_string(&self) -> String {
        general_purpose::STANDARD_NO_PAD
            .decode(self.content)
            .map(|d| self.enc.decode(d.as_slice()).0.to_string())
            .unwrap_or("".into())
    }
}

#[derive(PartialEq, Debug, Clone)]
pub struct QuotedWord<'a> {
    pub enc: &'static Encoding,
    pub chunks: Vec<QuotedChunk<'a>>,
}

impl<'a> QuotedWord<'a> {
    pub fn to_string(&self) -> String {
        self.chunks.iter().fold(String::new(), |mut acc, c| {
            match c {
                QuotedChunk::Safe(v) => {
                    let (content, _) = encoding_rs::UTF_8.decode_without_bom_handling(v);
                    acc.push_str(content.as_ref());
                }
                QuotedChunk::Space => acc.push(' '),
                QuotedChunk::Encoded(v) => {
                    let (d, _) = self.enc.decode_without_bom_handling(v.as_slice());
                    acc.push_str(d.as_ref());
                }
            };
            acc
        })
    }
}

#[derive(PartialEq, Debug, Clone)]
pub enum QuotedChunk<'a> {
    Safe(&'a [u8]),
    Encoded(Vec<u8>),
    Space,
}

//quoted_printable
pub fn ptext(input: &[u8]) -> IResult<&[u8], Vec<QuotedChunk>> {
    many0(alt((safe_char2, encoded_space, many_hex_octet)))(input)
}

fn safe_char2(input: &[u8]) -> IResult<&[u8], QuotedChunk> {
    map(take_while1(is_safe_char2), QuotedChunk::Safe)(input)
}

/// RFC2047 section 4.2
/// 8-bit values which correspond to printable ASCII characters other
/// than "=", "?", and "_" (underscore), MAY be represented as those
/// characters.
fn is_safe_char2(c: u8) -> bool {
    c >= ascii::SP && c != ascii::UNDERSCORE && c != ascii::QUESTION && c != ascii::EQ
}

fn encoded_space(input: &[u8]) -> IResult<&[u8], QuotedChunk> {
    map(tag("_"), |_| QuotedChunk::Space)(input)
}

fn hex_octet(input: &[u8]) -> IResult<&[u8], u8> {
    use nom::error::*;

    let (rest, hbytes) = preceded(tag("="), take(2usize))(input)?;

    let hstr = String::from_utf8_lossy(hbytes);
    let parsed = u8::from_str_radix(hstr.as_ref(), 16)
        .map_err(|_| nom::Err::Error(Error::new(input, ErrorKind::Verify)))?;

    Ok((rest, parsed))
}

fn many_hex_octet(input: &[u8]) -> IResult<&[u8], QuotedChunk> {
    map(many1(hex_octet), QuotedChunk::Encoded)(input)
}

//base64 (maybe use a crate)
pub fn btext(input: &[u8]) -> IResult<&[u8], &[u8]> {
    terminated(take_while(is_bchar), many0(tag("=")))(input)
}

fn is_bchar(c: u8) -> bool {
    is_alphanumeric(c) || c == ascii::PLUS || c == ascii::SLASH
}

#[cfg(test)]
mod tests {
    use super::*;

    // =?iso8859-1?Q?Accus=E9_de_r=E9ception_(affich=E9)?=
    #[test]
    fn test_ptext() {
        assert_eq!(
            ptext(b"Accus=E9_de_r=E9ception_(affich=E9)"),
            Ok((
                &b""[..],
                vec![
                    QuotedChunk::Safe(&b"Accus"[..]),
                    QuotedChunk::Encoded(vec![0xe9]),
                    QuotedChunk::Space,
                    QuotedChunk::Safe(&b"de"[..]),
                    QuotedChunk::Space,
                    QuotedChunk::Safe(&b"r"[..]),
                    QuotedChunk::Encoded(vec![0xe9]),
                    QuotedChunk::Safe(&b"ception"[..]),
                    QuotedChunk::Space,
                    QuotedChunk::Safe(&b"(affich"[..]),
                    QuotedChunk::Encoded(vec![0xe9]),
                    QuotedChunk::Safe(&b")"[..]),
                ]
            ))
        );
    }

    #[test]
    fn test_decode_word() {
        assert_eq!(
            encoded_word(b"=?iso8859-1?Q?Accus=E9_de_r=E9ception_(affich=E9)?=")
                .unwrap()
                .1
                .to_string(),
            "Accusé de réception (affiché)".to_string(),
        );
    }

    // =?ISO-8859-1?B?SWYgeW91IGNhbiByZWFkIHRoaXMgeW8=?=
    #[test]
    fn test_decode_word_b64() {
        assert_eq!(
            encoded_word(b"=?ISO-8859-1?B?SWYgeW91IGNhbiByZWFkIHRoaXMgeW8=?=")
                .unwrap()
                .1
                .to_string(),
            "If you can read this yo".to_string(),
        );
    }

    #[test]
    fn test_strange_quoted() {
        assert_eq!(
            encoded_word(b"=?UTF-8?Q?John_Sm=C3=AEth?=")
                .unwrap()
                .1
                .to_string(),
            "John Smîth".to_string(),
        );
    }
}
