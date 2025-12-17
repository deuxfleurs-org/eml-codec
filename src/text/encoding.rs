use bounded_static::{ToStatic, ToBoundedStatic, IntoBoundedStatic};
use encoding_rs::Encoding;

use base64::{engine::general_purpose, Engine as _};
use nom::{
    branch::alt,
    bytes::complete::{tag, take, take_while, take_while1},
    character::complete::one_of,
    character::is_alphanumeric,
    combinator::{map, opt},
    multi::{many0, many1},
    sequence::{delimited, preceded, terminated, tuple},
    IResult,
};
use std::borrow::Cow;

use crate::text::ascii;
use crate::text::whitespace::cfws;
use crate::text::words;

pub fn encoded_word(input: &[u8]) -> IResult<&[u8], EncodedWord<'_>> {
    delimited(opt(cfws), encoded_word_plain, opt(cfws))(input)
}

// NOTE: this is part of the comment syntax, so should not
// recurse and call CFWS itself, for parsing efficiency reasons.
pub fn encoded_word_plain(input: &[u8]) -> IResult<&[u8], EncodedWord<'_>> {
    alt((encoded_word_quoted, encoded_word_base64))(input)
}

pub fn encoded_word_quoted(input: &[u8]) -> IResult<&[u8], EncodedWord<'_>> {
    let (rest, (_, charset, _, _, _, txt, _)) = tuple((
        tag("=?"),
        words::mime_atom_plain,
        tag("?"),
        one_of("Qq"),
        tag("?"),
        ptext,
        tag("?="),
    ))(input)?;

    let renc = Encoding::for_label(charset).unwrap_or(encoding_rs::WINDOWS_1252);
    let parsed = EncodedWord::Quoted(QuotedWord {
        enc: renc,
        chunks: txt,
    });
    Ok((rest, parsed))
}

pub fn encoded_word_base64(input: &[u8]) -> IResult<&[u8], EncodedWord<'_>> {
    let (rest, (_, charset, _, _, _, txt, _)) = tuple((
        tag("=?"),
        words::mime_atom_plain,
        tag("?"),
        one_of("Bb"),
        tag("?"),
        btext,
        tag("?="),
    ))(input)?;

    let renc = Encoding::for_label(charset).unwrap_or(encoding_rs::WINDOWS_1252);
    let parsed = EncodedWord::Base64(Base64Word {
        enc: renc,
        content: Cow::Borrowed(txt),
    });
    Ok((rest, parsed))
}

#[derive(PartialEq, Debug, Clone, ToStatic)]
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
    pub content: Cow<'a, [u8]>,
}
impl ToBoundedStatic for Base64Word<'_> {
    type Static = Base64Word<'static>;
    fn to_static(&self) -> Base64Word<'static> {
        Base64Word { enc: self.enc, content: self.content.to_static() }
    }
}
impl IntoBoundedStatic for Base64Word<'_> {
    type Static = Base64Word<'static>;
    fn into_static(self) -> Base64Word<'static> {
        Base64Word { enc: self.enc, content: self.content.to_static() }
    }
}


impl<'a> Base64Word<'a> {
    pub fn to_string(&self) -> String {
        general_purpose::STANDARD_NO_PAD
            .decode(&self.content)
            .map(|d| self.enc.decode(d.as_slice()).0.to_string())
            .unwrap_or("".into())
    }
}

#[derive(PartialEq, Debug, Clone)]
pub struct QuotedWord<'a> {
    pub enc: &'static Encoding,
    pub chunks: Vec<QuotedChunk<'a>>,
}
impl ToBoundedStatic for QuotedWord<'_> {
    type Static = QuotedWord<'static>;
    fn to_static(&self) -> QuotedWord<'static> {
        QuotedWord { enc: self.enc, chunks: self.chunks.to_static() }
    }
}
impl<'a> IntoBoundedStatic for QuotedWord<'a> {
    type Static = QuotedWord<'static>;
    fn into_static(self) -> QuotedWord<'static> {
        QuotedWord { enc: self.enc, chunks: self.chunks.to_static() }
    }
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

#[derive(PartialEq, Debug, Clone, ToStatic)]
pub enum QuotedChunk<'a> {
    Safe(Cow<'a, [u8]>),
    Encoded(Vec<u8>),
    Space,
}

//quoted_printable
// XXX safe_char2 includes SPACE; is this really OK?
pub fn ptext(input: &[u8]) -> IResult<&[u8], Vec<QuotedChunk<'_>>> {
    many0(alt((safe_char2, encoded_space, many_hex_octet)))(input)
}

fn safe_char2(input: &[u8]) -> IResult<&[u8], QuotedChunk<'_>> {
    map(take_while1(is_safe_char2), |b| {
        QuotedChunk::Safe(Cow::Borrowed(b))
    })(input)
}

/// RFC2047 section 4.2
/// 8-bit values which correspond to printable ASCII characters other
/// than "=", "?", and "_" (underscore), MAY be represented as those
/// characters.
fn is_safe_char2(c: u8) -> bool {
    c >= ascii::SP && c != ascii::UNDERSCORE && c != ascii::QUESTION && c != ascii::EQ
}

fn encoded_space(input: &[u8]) -> IResult<&[u8], QuotedChunk<'_>> {
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

fn many_hex_octet(input: &[u8]) -> IResult<&[u8], QuotedChunk<'_>> {
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
                    QuotedChunk::Safe(b"Accus"[..].into()),
                    QuotedChunk::Encoded(vec![0xe9]),
                    QuotedChunk::Space,
                    QuotedChunk::Safe(b"de"[..].into()),
                    QuotedChunk::Space,
                    QuotedChunk::Safe(b"r"[..].into()),
                    QuotedChunk::Encoded(vec![0xe9]),
                    QuotedChunk::Safe(b"ception"[..].into()),
                    QuotedChunk::Space,
                    QuotedChunk::Safe(b"(affich"[..].into()),
                    QuotedChunk::Encoded(vec![0xe9]),
                    QuotedChunk::Safe(b")"[..].into()),
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
