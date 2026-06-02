#[cfg(feature = "arbitrary")]
use arbitrary::Arbitrary;
use bounded_static::ToStatic;

use base64::{engine::general_purpose, Engine as _};
use nom::{
    branch::alt,
    bytes::complete::{tag, take, take_while, take_while1},
    character::complete::one_of,
    character::is_alphanumeric,
    combinator::{all_consuming, map, map_parser, opt, recognize},
    multi::{many0, many1, separated_list1},
    sequence::{delimited, preceded, terminated, tuple},
    IResult,
};
use std::borrow::Cow;
use std::fmt;

use crate::i18n::ContainsUtf8;
use crate::print::{print_seq, Formatter, Print, ToStringFromPrint};
use crate::text::ascii;
use crate::text::charset::EmailCharset;
use crate::text::utf8::take_utf8_while1;
use crate::text::whitespace::{self, cfws, fws};
use crate::text::words;
#[cfg(feature = "arbitrary")]
use crate::{
    arbitrary_utils::{arbitrary_vec_nonempty, arbitrary_vec_where},
    fuzz_eq::FuzzEq,
};

// Context in which an encoded word is parsed.
//
// `Phrase` is more strict than `Comment`, which is more strict than `Unstructured`.
// ("more strict" == "allows less inputs")
#[derive(Clone, Copy)]
pub enum Context {
    Phrase,
    Comment,
    Unstructured,
}

pub fn encoded_word(ctx: Context) -> impl FnMut(&[u8]) -> IResult<&[u8], EncodedWord<'_>> {
    move |input| delimited(opt(cfws), encoded_word_plain(ctx), opt(cfws))(input)
}

// NOTE: this is used in the comment syntax, so should not
// recurse and call CFWS itself, for parsing efficiency reasons.
pub fn encoded_word_plain(ctx: Context) -> impl FnMut(&[u8]) -> IResult<&[u8], EncodedWord<'_>> {
    move |input| map(separated_list1(fws, encoded_word_token(ctx)), EncodedWord)(input)
}

pub fn encoded_word_token(
    ctx: Context,
) -> impl FnMut(&[u8]) -> IResult<&[u8], EncodedWordToken<'_>> {
    move |input| {
        // An encoded word is always a special case of an atom-like token. Which characters are
        // allowed in this atom token depends on the context, so we first read the atom, then try to
        // parse it fully as an encoded word.
        map_parser(
            // read an atom-like token
            encoded_word_token_atom(ctx),
            // ...which must fully represent an encoded word
            all_consuming(alt((encoded_word_token_quoted, encoded_word_token_base64))),
        )(input)
    }
}

fn encoded_word_token_atom(ctx: Context) -> impl FnMut(&[u8]) -> IResult<&[u8], &[u8]> {
    move |input| {
        // use `recognize` as this will be re-parsed by the encoded-word
        // combinators, and all our parsing combinators work on &[u8]s.
        //
        // XXX if invalid utf-8 is present, this makes `take_utf8_while1`
        // unnecessarily allocate a string for the result that is then
        // discarded.
        match ctx {
            // mirrors words::atom
            Context::Phrase => recognize(take_utf8_while1(words::is_atext))(input),
            // mirrors whitespace::ctext
            Context::Comment => recognize(take_utf8_while1(whitespace::is_ctext))(input),
            // mirrors misc_token::obs_utext_token (non-obs case)
            Context::Unstructured => recognize(take_utf8_while1(words::is_vchar))(input),
        }
    }
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

    let parsed = EncodedWordToken::Quoted(QuotedWord {
        enc: charset.0.into(),
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

    let parsed = EncodedWordToken::Base64(Base64Word {
        enc: charset.0.into(),
        content: Cow::Borrowed(txt),
    });
    Ok((rest, parsed))
}

/// Represents an encoded word.
#[derive(Clone, ContainsUtf8, Debug, PartialEq, ToStatic, ToStringFromPrint)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
#[contains_utf8(false)]
pub struct EncodedWord<'a>(pub Vec<EncodedWordToken<'a>>); // must be non-empty

impl<'a> EncodedWord<'a> {
    /// Returns the data represented by this `EncodedWord`, encoded into UTF8
    pub fn data(&self) -> String {
        self.0
            .iter()
            .map(|tok| tok.data())
            .collect::<Vec<_>>()
            .join("")
    }

    /// Build an encoded word from UTF-8 chars. Uses the UTF-8 charset and
    /// quoted encoding.
    pub fn from_chars<I>(chars: I) -> Self
    where
        I: IntoIterator<Item = char>,
    {
        const HEADER: &[u8] = b"=?UTF-8?Q?";
        const FOOTER: &[u8] = b"?=";
        // specified in RFC2047
        const MAX_LEN: usize = 75;

        let mut tokens: Vec<EncodedWordToken> = vec![];
        let mut cur_chunks: Vec<QuotedChunk> = vec![];
        let mut cur_word_len = 0;
        let mut char_bytes: [u8; 4] = [0; 4];

        for c in chars {
            if HEADER.len() + cur_word_len + FOOTER.len() > MAX_LEN - 3
            /* max size minus room for the next encoded byte */
            {
                let mut w = QuotedWord {
                    enc: EmailCharset::utf8(),
                    chunks: vec![],
                };
                std::mem::swap(&mut w.chunks, &mut cur_chunks);
                tokens.push(EncodedWordToken::Quoted(w));
                cur_word_len = 0;
            }

            if c.is_ascii() && is_qchar_safe_strict(c as u8) {
                if let Some(QuotedChunk::Safe(s)) = cur_chunks.last_mut() {
                    let s = s.to_mut();
                    s.push(c as u8)
                } else {
                    cur_chunks.push(QuotedChunk::Safe(vec![c as u8].into()));
                }
                cur_word_len += 1;
            } else if c == char::from(ascii::SP) {
                // space has a special treatment (RFC2047, 4.2, (2))
                cur_chunks.push(QuotedChunk::Space);
                cur_word_len += 1;
            } else {
                c.encode_utf8(&mut char_bytes);
                let c_bytes = &char_bytes[0..c.len_utf8()];
                if let Some(QuotedChunk::Encoded(e)) = cur_chunks.last_mut() {
                    e.extend_from_slice(c_bytes)
                } else {
                    cur_chunks.push(QuotedChunk::Encoded(c_bytes.to_vec()))
                }
                // each encoded byte uses three characters (=XX)
                cur_word_len += 3 * c.len_utf8();
            }
        }

        tokens.push(EncodedWordToken::Quoted(QuotedWord {
            enc: EmailCharset::utf8(),
            chunks: cur_chunks,
        }));

        EncodedWord(tokens)
    }
}
impl<'a> Print for EncodedWord<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        print_seq(fmt, &self.0, Formatter::write_fws)
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for EncodedWord<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(EncodedWord(arbitrary_vec_nonempty(u)?))
    }
}

#[derive(PartialEq, Debug, Clone, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq, Arbitrary))]
pub enum EncodedWordToken<'a> {
    Quoted(QuotedWord<'a>),
    Base64(Base64Word<'a>),
}
impl<'a> EncodedWordToken<'a> {
    pub fn data(&self) -> String {
        match self {
            EncodedWordToken::Quoted(v) => v.data(),
            EncodedWordToken::Base64(v) => v.data(),
        }
    }
}
impl<'a> Print for EncodedWordToken<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        match self {
            EncodedWordToken::Quoted(q) => q.print(fmt),
            EncodedWordToken::Base64(b) => b.print(fmt),
        }
    }
}

#[derive(PartialEq, Clone, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub struct Base64Word<'a> {
    pub enc: EmailCharset,
    // `content` must represent base64-encoded data. In particular,
    // all bytes in `content` must satisfy `is_bchar`.
    #[cfg_attr(feature = "arbitrary", fuzz_eq(use_eq))]
    pub content: Cow<'a, [u8]>,
}
impl<'a> fmt::Debug for Base64Word<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Base64Word")
            .field("enc", &self.enc)
            .field("content", &String::from_utf8_lossy(&self.content))
            .finish()
    }
}

impl<'a> Base64Word<'a> {
    pub fn data(&self) -> String {
        general_purpose::STANDARD_NO_PAD
            .decode(&self.content)
            .map(|d| self.enc.decode(d.as_slice()).to_string())
            .unwrap_or("".into())
    }
}

impl<'a> Print for Base64Word<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        fmt.write_bytes(b"=?");
        fmt.write_bytes(self.enc.as_bytes());
        fmt.write_bytes(b"?B?");
        fmt.write_bytes(&self.content);
        fmt.write_bytes(b"?=");
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for Base64Word<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Base64Word<'a>> {
        let enc: EmailCharset = u.arbitrary()?;
        let content = arbitrary_vec_where(u, |c| is_bchar(*c))?;
        Ok(Base64Word {
            enc,
            content: Cow::Owned(content),
        })
    }
}

#[derive(PartialEq, Debug, Clone, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
pub struct QuotedWord<'a> {
    pub enc: EmailCharset,
    pub chunks: Vec<QuotedChunk<'a>>,
}

impl<'a> QuotedWord<'a> {
    pub fn data(&self) -> String {
        self.chunks.iter().fold(String::new(), |mut acc, c| {
            match c {
                QuotedChunk::Safe(v) => {
                    let (content, _) = encoding_rs::UTF_8.decode_without_bom_handling(v);
                    acc.push_str(content.as_ref());
                }
                QuotedChunk::Space => acc.push(' '),
                QuotedChunk::Encoded(v) => {
                    let d = self.enc.decode(v.as_slice());
                    acc.push_str(d.as_ref());
                }
            };
            acc
        })
    }
}

impl<'a> Print for QuotedWord<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        fmt.write_bytes(b"=?");
        fmt.write_bytes(self.enc.as_bytes());
        fmt.write_bytes(b"?Q?");
        print_seq(fmt, &self.chunks, |_| ());
        fmt.write_bytes(b"?=");
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> FuzzEq for QuotedWord<'a> {
    fn fuzz_eq(&self, other: &QuotedWord<'a>) -> bool {
        self.enc.fuzz_eq(&other.enc)
            && normalize_quoted_chunks(&self.chunks) == normalize_quoted_chunks(&other.chunks)
    }
}

#[derive(PartialEq, Clone, ToStatic)]
pub enum QuotedChunk<'a> {
    Safe(Cow<'a, [u8]>), // must satisfy `is_safe_char2`
    Encoded(Vec<u8>),
    Space,
}
impl<'a> fmt::Debug for QuotedChunk<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QuotedChunk::Safe(b) => fmt
                .debug_tuple("QuotedChunk::Safe")
                .field(&String::from_utf8_lossy(b))
                .finish(),
            QuotedChunk::Encoded(e) => fmt.debug_tuple("QuotedChunk::Encoded").field(e).finish(),
            QuotedChunk::Space => fmt.debug_tuple("QuotedChunk::Space").finish(),
        }
    }
}

impl<'a> Print for QuotedChunk<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        match self {
            QuotedChunk::Safe(b) => fmt.write_bytes(b),
            QuotedChunk::Encoded(e) => {
                for c in e {
                    fmt.write_bytes(format!("={:02X}", c).as_bytes());
                }
            }
            QuotedChunk::Space => fmt.write_bytes(b"_"),
        }
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for QuotedChunk<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<QuotedChunk<'a>> {
        match u.int_in_range(0..=2)? {
            0 => {
                let v = arbitrary_vec_where(u, |c| is_safe_char2(*c))?;
                Ok(QuotedChunk::Safe(Cow::Owned(v)))
            }
            1 => {
                let v: Vec<u8> = u.arbitrary()?;
                Ok(QuotedChunk::Encoded(v))
            }
            2 => Ok(QuotedChunk::Space),
            _ => unreachable!(),
        }
    }
}

//quoted_printable
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
    words::is_vchar(c.into()) && c != ascii::UNDERSCORE && c != ascii::QUESTION && c != ascii::EQ
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
//TODO: this strips off padding chars (final '='s). is this ok?
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
    is_alphanumeric(b)
        || b == ascii::EXCLAMATION
        || b == ascii::ASTERISK
        || b == ascii::PLUS
        || b == ascii::MINUS
        || b == ascii::SLASH
}

#[cfg(feature = "arbitrary")]
fn normalize_quoted_chunks<'a>(chunks: &Vec<QuotedChunk<'a>>) -> Vec<QuotedChunk<'static>> {
    use bounded_static::ToBoundedStatic;
    let mut new_chunks: Vec<QuotedChunk<'static>> = vec![];
    for chunk in chunks {
        match (new_chunks.last_mut(), chunk) {
            (Some(QuotedChunk::Safe(b1)), QuotedChunk::Safe(b2)) => b1.to_mut().extend(&**b2),
            (Some(QuotedChunk::Encoded(v1)), QuotedChunk::Encoded(v2)) => v1.extend(v2),
            (_, _) => new_chunks.push(chunk.to_static()),
        }
    }
    new_chunks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::print::tests::print_to_vec_with;

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
    fn test_invalid_space() {
        // Context::Unstructured is the most lenient
        assert!(
            encoded_word(Context::Unstructured)(b"=?iso8859-1?Q?Accus=E9 de r=E9ception?=")
                .is_err()
        );
    }

    #[test]
    fn test_decode_word() {
        // This is only parsable in the Unstructured context, because of the naked parenthesis
        assert_eq!(
            encoded_word(Context::Unstructured)(
                b"=?iso8859-1?Q?Accus=E9_de_r=E9ception_(affich=E9)?="
            )
            .unwrap()
            .1
            .data(),
            "Accusé de réception (affiché)".to_string(),
        );

        assert_eq!(
            encoded_word(Context::Unstructured)(b"=?iso-8859-1?Q?=805.4bn?=")
                .unwrap()
                .1
                .data(),
            "€5.4bn".to_string(),
        );

        assert!(encoded_word(Context::Phrase)(
            b"=?iso8859-1?Q?Accus=E9_de_r=E9ception_(affich=E9)?="
        )
        .is_err());
    }

    #[test]
    fn test_decode_word_ast() {
        assert_eq!(
            encoded_word(Context::Phrase)(b"=?ISO-8859-1?B?SWYgeW91IGNhbiByZWFkIHRoaXMgeW8=?=")
                .unwrap()
                .1,
            EncodedWord(vec![EncodedWordToken::Base64(Base64Word {
                enc: EmailCharset::from(b"iso-8859-1"),
                content: b"SWYgeW91IGNhbiByZWFkIHRoaXMgeW8"[..].into(),
            })])
        );
    }

    // =?ISO-8859-1?B?SWYgeW91IGNhbiByZWFkIHRoaXMgeW8=?=
    #[test]
    fn test_decode_word_b64() {
        assert_eq!(
            encoded_word(Context::Phrase)(b"=?ISO-8859-1?B?SWYgeW91IGNhbiByZWFkIHRoaXMgeW8=?=")
                .unwrap()
                .1
                .data(),
            "If you can read this yo".to_string(),
        );
    }

    #[test]
    fn test_strange_quoted() {
        assert_eq!(
            encoded_word(Context::Phrase)(b"=?UTF-8?Q?John_Sm=C3=AEth?=")
                .unwrap()
                .1
                .data(),
            "John Smîth".to_string(),
        );
    }

    #[test]
    fn test_multiple() {
        // white space between adjacent encoded word is not displayed
        assert_eq!(
            encoded_word(Context::Phrase)(b"=?ISO-8859-1?Q?a?= =?ISO-8859-1?Q?b?=")
                .unwrap()
                .1
                .data(),
            "ab".to_string(),
        );

        assert_eq!(
            encoded_word(Context::Phrase)(b"=?ISO-8859-1?Q?a?=  \r\n   =?ISO-8859-1?Q?b?=")
                .unwrap()
                .1
                .data(),
            "ab".to_string(),
        );
    }

    #[test]
    fn test_encode() {
        let out = print_to_vec_with(|f| {
            EncodedWord::from_chars("Accusé de réception (affiché)".chars()).print(f);
        });
        assert_eq!(
            String::from_utf8_lossy(&out),
            "=?UTF-8?Q?Accus=C3=A9_de_r=C3=A9ception_=28affich=C3=A9=29?="
        );

        let out = print_to_vec_with(|f| {
            EncodedWord::from_chars("John Smîth".chars()).print(f);
        });
        assert_eq!(out, b"=?UTF-8?Q?John_Sm=C3=AEth?=");
    }

    #[test]
    fn test_encode_folding() {
        let out = print_to_vec_with(|f| {
            f.begin_line_folding();
            EncodedWord::from_chars(
                "Accusé de réception (affiché) Accusé de réception (affiché)".chars(),
            )
            .print(f);
        });
        assert_eq!(
            String::from_utf8_lossy(&out),
            "=?UTF-8?Q?Accus=C3=A9_de_r=C3=A9ception_=28affich=C3=A9=29_Accus=C3=A9_?=\r\n =?UTF-8?Q?de_r=C3=A9ception_=28affich=C3=A9=29?="
        );
    }

    #[test]
    fn test_encode_empty() {
        let out = print_to_vec_with(|f| {
            EncodedWord::from_chars("".chars()).print(f);
        });
        assert_eq!(out, b"=?UTF-8?Q??=");
    }
}
