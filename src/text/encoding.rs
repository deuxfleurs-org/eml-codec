use bounded_static::{ToStatic, ToBoundedStatic, IntoBoundedStatic};
use encoding_rs::Encoding;

use base64::{engine::general_purpose, Engine as _};
use nom::{
    branch::alt,
    bytes::complete::{tag, take, take_while, take_while1},
    character::complete::one_of,
    character::is_alphanumeric,
    combinator::{map, opt},
    multi::{many0, many1, separated_list1},
    sequence::{delimited, preceded, terminated, tuple},
    IResult,
};
use std::borrow::Cow;
use std::io::Write;

use crate::print::{print_seq, Print, Formatter};
use crate::text::ascii;
use crate::text::whitespace::{cfws, fws};
use crate::text::words;

// XXX: the parser below does not implement the spec stricty.
// Specifically, it is more lenient than the spec in what it accepts
// inside of an encoded word. In particular:
// - it allows characters that are always explicitly forbidden (e.g. space);
// - it is not aware of the context in which the encoded word
//   appears, which can cause more characters to be forbidden (e.g.
//   "(" and ")" are forbidden inside of a comment).
//
// At this point it is not clear whether strictly implementing the spec
// in the parser is a good or bad thing (since we also want to be resilient
// to incorrect input, on a best-effort basis).
//
// The printer is, in any case, strictly spec compliant.

pub fn encoded_word(input: &[u8]) -> IResult<&[u8], EncodedWord<'_>> {
    delimited(opt(cfws), encoded_word_plain, opt(cfws))(input)
}

// NOTE: this is used in the comment syntax, so should not
// recurse and call CFWS itself, for parsing efficiency reasons.
pub fn encoded_word_plain(input: &[u8]) -> IResult<&[u8], EncodedWord<'_>> {
    map(separated_list1(fws, encoded_word_token), EncodedWord)(input)
}

pub fn encoded_word_token(input: &[u8]) -> IResult<&[u8], EncodedWordToken<'_>> {
    alt((encoded_word_token_quoted, encoded_word_token_base64))(input)
}

pub fn encoded_word_token_quoted(input: &[u8]) -> IResult<&[u8], EncodedWordToken<'_>> {
    let (rest, (_, charset, _, _, _, txt, _)) = tuple((
        tag("=?"),
        words::mime_atom_plain,
        tag("?"),
        one_of("Qq"),
        tag("?"),
        ptext,
        tag("?="),
    ))(input)?;

    // NOTE: we use encoding_rs directly instead of crate::mime::charset, because
    // we only care about decoding (in `to_string`); printing will then always use
    // UTF-8 as output charset.
    let renc = Encoding::for_label(charset).unwrap_or(encoding_rs::WINDOWS_1252);
    let parsed = EncodedWordToken::Quoted(QuotedWord {
        enc: renc,
        chunks: txt,
    });
    Ok((rest, parsed))
}

pub fn encoded_word_token_base64(input: &[u8]) -> IResult<&[u8], EncodedWordToken<'_>> {
    let (rest, (_, charset, _, _, _, txt, _)) = tuple((
        tag("=?"),
        words::mime_atom_plain,
        tag("?"),
        one_of("Bb"),
        tag("?"),
        btext,
        tag("?="),
    ))(input)?;

    // NOTE: we use encoding_rs and not crate::mime::charset; see above.
    let renc = Encoding::for_label(charset).unwrap_or(encoding_rs::WINDOWS_1252);
    let parsed = EncodedWordToken::Base64(Base64Word {
        enc: renc,
        content: Cow::Borrowed(txt),
    });
    Ok((rest, parsed))
}

#[derive(PartialEq, Debug, Clone, ToStatic)]
pub struct EncodedWord<'a>(pub Vec<EncodedWordToken<'a>>);

impl<'a> EncodedWord<'a> {
    pub fn to_string(&self) -> String {
        self.0.iter().map(|tok| tok.to_string()).collect::<Vec<_>>().join("")
    }
}
impl<'a> Print for EncodedWord<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        print_seq(fmt, &self.0, Formatter::write_fws)
    }
}

#[derive(PartialEq, Debug, Clone, ToStatic)]
pub enum EncodedWordToken<'a> {
    Quoted(QuotedWord<'a>),
    Base64(Base64Word<'a>),
}
impl<'a> EncodedWordToken<'a> {
    pub fn to_string(&self) -> String {
        match self {
            EncodedWordToken::Quoted(v) => v.to_string(),
            EncodedWordToken::Base64(v) => v.to_string(),
        }
    }
}
impl<'a> Print for EncodedWordToken<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        print_utf8_encoded(fmt, self.to_string().chars())
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

// Returns whether ASCII char `b` is safe to display as-is in the
// encoded-text of an encoded-word.
// As per RFC2047, in general this depends on the context in which
// this encoded-word occurs. Because this function is used for
// printing, it returns the most conservative answer, i.e. it only
// returns `true` if the character is safe to use in any context.
fn is_qchar_safe_strict(b: u8) -> bool {
    // General restrictions for the Q encoding (RFC2047, 4.2, (3)),
    // + restrictions when inside a comment (RFC2047, 5, (2)),
    // + restrictions when inside a phrase (RFC2047, 5, (3)).
    is_alphanumeric(b) ||
        b == ascii::EXCLAMATION ||
        b == ascii::ASTERISK ||
        b == ascii::PLUS ||
        b == ascii::MINUS ||
        b == ascii::SLASH
}

// XXX: how can we enforce that encoded words are always preceded with linear
// space (or beginning of header body)?
pub fn print_utf8_encoded<I>(fmt: &mut impl Formatter, data: I)
where
    I: IntoIterator<Item = char>
{
    const HEADER: &[u8] = b"=?UTF-8?Q?";
    const FOOTER: &[u8] = b"?=";
    const MAX_LEN: usize = 75; // specified in RFC2047

    let mut buf: Vec<u8> = Vec::with_capacity(MAX_LEN);
    let mut char_bytes: [u8; 4] = [0; 4];
    let mut char_encoded: Vec<u8> = Vec::new();

    for c in data {
        if c.is_ascii() && is_qchar_safe_strict(c as u8) {
            char_encoded.push(c as u8);
        } else if c == char::from(ascii::SP) {
            // space has a special treatment (RFC2047, 4.2, (2))
            char_encoded.push(ascii::UNDERSCORE);
        } else {
            c.encode_utf8(&mut char_bytes);
            for i in 0..c.len_utf8() {
                write!(&mut char_encoded, "={:02X}", char_bytes[i]).unwrap();
            }
        }

        if HEADER.len()
            + buf.len()
            + char_encoded.len()
            + FOOTER.len() > MAX_LEN
        {
            fmt.write_bytes(HEADER);
            fmt.write_bytes(&buf);
            fmt.write_bytes(FOOTER);
            fmt.write_fws();
            buf.truncate(0);
        }

        buf.extend_from_slice(&char_encoded);
        char_encoded.truncate(0);
    }

    // write any leftover data in buf
    if !buf.is_empty() {
        fmt.write_bytes(HEADER);
        fmt.write_bytes(&buf);
        fmt.write_bytes(FOOTER);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::print::tests::with_formatter;

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

    #[test]
    fn test_multiple() {
        // white space between adjacent encoded word is not displayed
        assert_eq!(
            encoded_word(b"=?ISO-8859-1?Q?a?= =?ISO-8859-1?Q?b?=")
                .unwrap()
                .1
                .to_string(),
            "ab".to_string(),
        );

        assert_eq!(
            encoded_word(b"=?ISO-8859-1?Q?a?=  \r\n   =?ISO-8859-1?Q?b?=")
                .unwrap()
                .1
                .to_string(),
            "ab".to_string(),
        );
    }

    #[test]
    fn test_encode() {
        let out = with_formatter(|f| {
            print_utf8_encoded(f, "Accusé de réception (affiché)".chars());
        });
        assert_eq!(
            out,
            b"=?UTF-8?Q?Accus=C3=A9_de_r=C3=A9ception_=28affich=C3=A9=29?="
        );

        let out = with_formatter(|f| {
            print_utf8_encoded(f, "John Smîth".chars());
        });
        assert_eq!(
            out,
            b"=?UTF-8?Q?John_Sm=C3=AEth?="
        );
    }

    #[test]
    fn test_encode_folding() {
        let out = with_formatter(|f| {
            f.begin_line_folding();
            print_utf8_encoded(f, "Accusé de réception (affiché) Accusé de réception (affiché)".chars());
        });
        assert_eq!(
            out,
            b"=?UTF-8?Q?Accus=C3=A9_de_r=C3=A9ception_=28affich=C3=A9=29_Accus=C3=A9_de?=\r\n =?UTF-8?Q?_r=C3=A9ception_=28affich=C3=A9=29?="
        );
    }
}
