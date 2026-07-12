use bounded_static::ToStatic;
#[cfg(feature = "arbitrary")]
use {
    arbitrary::Arbitrary,
    eyre::{eyre, Context as _},
};

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
use crate::print::{self, print_iter, Formatter, Print};
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

/// Context in which an encoded word is parsed.
///
/// `Phrase` is more strict than `Comment`, which is more strict than `Unstructured`.
/// ("more strict" == "allows less inputs").
///
/// `<` corresponds to the "strictness" relation, where c1 < c2 means that c1 is
/// more strict (allows less inputs).
///
/// We have: `Phrase < Comment < Unstructured`
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
pub enum Context {
    Phrase,
    Comment,
    Unstructured,
}

/// Wrapper type used for printing an `EncodedWord` depending on the given context.
///
/// Printing (like parsing) is context-dependent. This is handled by not
/// implementing `Print` for `EncodedWord` itself, but `InContext<EncodedWord>`.
/// You can use the `in_context` method on `EncodedWord` to obtain an
/// `InContext` value that can be printed.
#[derive(Clone, Copy, Debug)]
pub struct InContext<T> {
    pub ctx: Context,
    pub v: T,
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
        qtext,
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
#[derive(Clone, ContainsUtf8, Debug, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
#[contains_utf8(false)]
pub struct EncodedWord<'a>(pub Vec<EncodedWordToken<'a>>); // must be non-empty

impl EncodedWord<'_> {
    /// Returns the data represented by this `EncodedWord`, encoded into UTF8
    pub fn data(&self) -> String {
        self.0
            .iter()
            .map(EncodedWordToken::data)
            .collect::<String>()
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

            // For simplicity, we only output unencoded "safe" tokens only for
            // chars that are safe in all contexts (the `Phrase` context is the
            // most restrictive).
            if c.is_ascii() && is_safe_qchar_phrase(c as u8) {
                if let Some(QuotedChunk::Safe(_, s)) = cur_chunks.last_mut() {
                    let s = s.to_mut();
                    s.push(c as u8);
                } else {
                    cur_chunks.push(QuotedChunk::Safe(Context::Phrase, vec![c as u8].into()));
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
                    e.extend_from_slice(c_bytes);
                } else {
                    cur_chunks.push(QuotedChunk::Encoded(c_bytes.to_vec()));
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

    pub fn in_context(&self, ctx: Context) -> InContext<&Self> {
        InContext { ctx, v: self }
    }
}
impl Print for InContext<&EncodedWord<'_>> {
    fn print(&self, fmt: &mut impl Formatter) {
        print_iter(
            fmt,
            self.v.0.iter().map(|tok| tok.in_context(self.ctx)),
            Formatter::write_fws,
        );
    }
}
impl ToString for InContext<&EncodedWord<'_>> {
    fn to_string(&self) -> String {
        print::print_to_string(&print::FMT_NOFOLD, self)
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
impl EncodedWordToken<'_> {
    pub fn data(&self) -> String {
        match self {
            EncodedWordToken::Quoted(v) => v.data(),
            EncodedWordToken::Base64(v) => v.data(),
        }
    }

    pub fn in_context(&self, ctx: Context) -> InContext<&Self> {
        InContext { ctx, v: self }
    }
}
impl Print for InContext<&EncodedWordToken<'_>> {
    fn print(&self, fmt: &mut impl Formatter) {
        match &self.v {
            EncodedWordToken::Quoted(q) => q.in_context(self.ctx).print(fmt),
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
impl fmt::Debug for Base64Word<'_> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Base64Word")
            .field("enc", &self.enc)
            .field("content", &String::from_utf8_lossy(&self.content))
            .finish()
    }
}

impl Base64Word<'_> {
    pub fn data(&self) -> String {
        general_purpose::STANDARD_NO_PAD
            .decode(&self.content)
            .map(|d| self.enc.decode(d.as_slice()).to_string())
            .unwrap_or_default()
    }
}

impl Print for Base64Word<'_> {
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
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
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

impl QuotedWord<'_> {
    pub fn data(&self) -> String {
        self.enc.decode(&self.unencoded_bytes()).to_string()
    }

    fn unencoded_bytes(&self) -> Vec<u8> {
        self.chunks.iter().fold(vec![], |mut acc, c| {
            match c {
                QuotedChunk::Safe(_, v) => acc.extend(v.as_ref()),
                QuotedChunk::Space => acc.push(b' '),
                QuotedChunk::Encoded(v) => acc.extend(v),
            }
            acc
        })
    }

    pub fn in_context(&self, ctx: Context) -> InContext<&Self> {
        InContext { ctx, v: self }
    }
}

impl Print for InContext<&QuotedWord<'_>> {
    fn print(&self, fmt: &mut impl Formatter) {
        fmt.write_bytes(b"=?");
        fmt.write_bytes(self.v.enc.as_bytes());
        fmt.write_bytes(b"?Q?");
        print_iter(
            fmt,
            self.v.chunks.iter().map(|c| c.in_context(self.ctx)),
            |_| (),
        );
        fmt.write_bytes(b"?=");
    }
}

#[cfg(feature = "arbitrary")]
impl FuzzEq for QuotedWord<'_> {
    fn fuzz_eq(&self, other: &Self) -> eyre::Result<()> {
        self.enc.fuzz_eq(&other.enc).context("QuotedWord.enc")?;
        let bytes1 = self.unencoded_bytes();
        let bytes2 = other.unencoded_bytes();
        if bytes1 != bytes2 {
            Err(eyre!(
                "QuotedWord bytes mismatch: {:?} vs {:?}",
                String::from_utf8_lossy(&bytes1),
                String::from_utf8_lossy(&bytes2),
            ))
        } else {
            Ok(())
        }
    }
}

#[derive(PartialEq, Clone, ToStatic)]
pub enum QuotedChunk<'a> {
    Safe(Context, Cow<'a, [u8]>),
    Encoded(Vec<u8>),
    Space,
}
impl QuotedChunk<'_> {
    pub fn in_context(&self, ctx: Context) -> InContext<&Self> {
        InContext { ctx, v: self }
    }
}

impl fmt::Debug for QuotedChunk<'_> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QuotedChunk::Safe(ctx, b) => fmt
                .debug_tuple("QuotedChunk::Safe")
                .field(ctx)
                .field(&String::from_utf8_lossy(b))
                .finish(),
            QuotedChunk::Encoded(e) => fmt.debug_tuple("QuotedChunk::Encoded").field(e).finish(),
            QuotedChunk::Space => fmt.debug_tuple("QuotedChunk::Space").finish(),
        }
    }
}

impl Print for InContext<&QuotedChunk<'_>> {
    fn print(&self, fmt: &mut impl Formatter) {
        match self.v {
            QuotedChunk::Safe(safe_ctx, b) => {
                if safe_ctx <= &self.ctx {
                    // if our enclosing context is less restrictive than the
                    // context the chunk is safe for, then we can print directly
                    fmt.write_bytes(b);
                } else {
                    // otherwise print as encoded bytes
                    for c in b.as_ref() {
                        fmt.write_bytes(format!("={c:02X}").as_bytes());
                    }
                }
            }
            QuotedChunk::Encoded(e) => {
                for c in e {
                    fmt.write_bytes(format!("={c:02X}").as_bytes());
                }
            }
            QuotedChunk::Space => fmt.write_bytes(b"_"),
        }
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for QuotedChunk<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        match u.int_in_range(0..=2)? {
            0 => {
                let ctx: Context = u.arbitrary()?;
                let v = match ctx {
                    Context::Phrase => arbitrary_vec_where(u, |c| is_safe_qchar_phrase(*c))?,
                    Context::Comment => arbitrary_vec_where(u, |c| is_safe_qchar_comment(*c))?,
                    Context::Unstructured => {
                        arbitrary_vec_where(u, |c| is_safe_qchar_unstructured(*c))?
                    }
                };
                Ok(QuotedChunk::Safe(ctx, Cow::Owned(v)))
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
pub fn qtext(input: &[u8]) -> IResult<&[u8], Vec<QuotedChunk<'_>>> {
    many0(alt((
        safe_qchars_phrase,
        safe_qchars_comment,
        safe_qchars_unstructured,
        encoded_space,
        many_hex_octet,
    )))(input)
}

fn safe_qchars_phrase(input: &[u8]) -> IResult<&[u8], QuotedChunk<'_>> {
    map(take_while1(is_safe_qchar_phrase), |b| {
        QuotedChunk::Safe(Context::Phrase, Cow::Borrowed(b))
    })(input)
}

fn safe_qchars_comment(input: &[u8]) -> IResult<&[u8], QuotedChunk<'_>> {
    map(take_while1(is_safe_qchar_comment), |b| {
        QuotedChunk::Safe(Context::Comment, Cow::Borrowed(b))
    })(input)
}

fn safe_qchars_unstructured(input: &[u8]) -> IResult<&[u8], QuotedChunk<'_>> {
    map(take_while1(is_safe_qchar_unstructured), |b| {
        QuotedChunk::Safe(Context::Unstructured, Cow::Borrowed(b))
    })(input)
}

/// RFC2047:
/// - section 4.2
///   8-bit values which correspond to printable ASCII characters other
///   than "=", "?", and "_" (underscore), MAY be represented as those
///   characters.
/// - section 5
///   unstructured tokens can use encoded words without further restrictions
fn is_safe_qchar_unstructured(c: u8) -> bool {
    words::is_vchar_ascii(c) && c != ascii::UNDERSCORE && c != ascii::QUESTION && c != ascii::EQ
}

/// RFC2047:
/// a "Q"-encoded 'encoded-word' which appears in a 'comment' MUST NOT contain
/// the characters "(", ")" or "\".
fn is_safe_qchar_comment(c: u8) -> bool {
    c != ascii::LEFT_PAR
        && c != ascii::RIGHT_PAR
        && c != ascii::BACKSLASH
        && is_safe_qchar_unstructured(c)
}

/// RFC2047:
/// in a phrase, the set of characters is restricted to:
/// <upper and lower case ASCII
/// letters, decimal digits, "!", "*", "+", "-", "/", "=" (only if
/// followed by two hexadecimal digits), and "_" (underscore, ASCII 95.)>.
/// (note: the last two cases are handled separately by `encoded_space` and `hex_octet`)
fn is_safe_qchar_phrase(c: u8) -> bool {
    c.is_ascii_alphanumeric()
        || c == ascii::EXCLAMATION
        || c == ascii::ASTERISK
        || c == ascii::PLUS
        || c == ascii::MINUS
        || c == ascii::SLASH
}

fn encoded_space(input: &[u8]) -> IResult<&[u8], QuotedChunk<'_>> {
    map(tag("_"), |_| QuotedChunk::Space)(input)
}

fn hex_octet(input: &[u8]) -> IResult<&[u8], u8> {
    let (rest, hbytes) = preceded(tag("="), take(2usize))(input)?;

    let hstr = String::from_utf8_lossy(hbytes);
    let parsed = u8::from_str_radix(hstr.as_ref(), 16).map_err(|_| {
        nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Verify))
    })?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::print::tests::print_to_vec_with;

    // =?iso8859-1?Q?Accus=E9_de_r=E9ception_(affich=E9)?=
    #[test]
    fn test_qtext() {
        assert_eq!(
            qtext(b"Accus=E9_de_r=E9ception_(affich=E9)"),
            Ok((
                &b""[..],
                vec![
                    QuotedChunk::Safe(Context::Phrase, b"Accus"[..].into()),
                    QuotedChunk::Encoded(vec![0xe9]),
                    QuotedChunk::Space,
                    QuotedChunk::Safe(Context::Phrase, b"de"[..].into()),
                    QuotedChunk::Space,
                    QuotedChunk::Safe(Context::Phrase, b"r"[..].into()),
                    QuotedChunk::Encoded(vec![0xe9]),
                    QuotedChunk::Safe(Context::Phrase, b"ception"[..].into()),
                    QuotedChunk::Space,
                    QuotedChunk::Safe(Context::Unstructured, b"(affich"[..].into()),
                    QuotedChunk::Encoded(vec![0xe9]),
                    QuotedChunk::Safe(Context::Unstructured, b")"[..].into()),
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
            EncodedWord::from_chars("Accusé de réception (affiché)".chars())
                .in_context(Context::Unstructured)
                .print(f);
        });
        assert_eq!(
            String::from_utf8_lossy(&out),
            "=?UTF-8?Q?Accus=C3=A9_de_r=C3=A9ception_=28affich=C3=A9=29?="
        );

        let out = print_to_vec_with(|f| {
            EncodedWord::from_chars("John Smîth".chars())
                .in_context(Context::Unstructured)
                .print(f);
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
            .in_context(Context::Unstructured)
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
            EncodedWord::from_chars("".chars())
                .in_context(Context::Phrase)
                .print(f);
        });
        assert_eq!(out, b"=?UTF-8?Q??=");
    }
}
