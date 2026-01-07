use bounded_static::ToStatic;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_while1},
    character::complete::space0,
    combinator::{map, opt},
    multi::{many0, many1, separated_list1},
    sequence::preceded,
    IResult,
};
use std::borrow::Cow;
use std::fmt;

use crate::text::{
    ascii,
    encoding::{self, encoded_word},
    quoted::{quoted_string, QuotedString},
    whitespace::{fws, is_obs_no_ws_ctl},
    words::{atom, is_vchar, mime_atom},
};

#[derive(Debug, PartialEq, Default, ToStatic)]
pub struct PhraseList<'a>(pub Vec<Phrase<'a>>);
pub fn phrase_list(input: &[u8]) -> IResult<&[u8], PhraseList<'_>> {
    map(separated_list1(tag(","), phrase), PhraseList)(input)
}

#[derive(Debug, PartialEq, Clone, ToStatic)]
pub enum MIMEWord<'a> {
    Quoted(QuotedString<'a>),
    Atom(Cow<'a, [u8]>),
}
impl Default for MIMEWord<'static> {
    fn default() -> Self {
        Self::Atom(Cow::Owned(vec![]))
    }
}
impl<'a> MIMEWord<'a> {
    pub fn to_string(&self) -> String {
        match self {
            Self::Quoted(v) => v.to_string(),
            Self::Atom(v) => encoding_rs::UTF_8
                .decode_without_bom_handling(v)
                .0
                .to_string(),
        }
    }
}
pub fn mime_word(input: &[u8]) -> IResult<&[u8], MIMEWord<'_>> {
    alt((
        map(quoted_string, MIMEWord::Quoted),
        map(mime_atom, |a| MIMEWord::Atom(Cow::Borrowed(a))),
    ))(input)
}

#[derive(PartialEq, ToStatic)]
pub enum Word<'a> {
    Quoted(QuotedString<'a>),
    Encoded(encoding::EncodedWord<'a>),
    Atom(Cow<'a, [u8]>),
}

impl<'a> ToString for Word<'a> {
    fn to_string(&self) -> String {
        match self {
            Word::Quoted(v) => v.to_string(),
            Word::Encoded(v) => v.to_string(),
            Word::Atom(v) => encoding_rs::UTF_8
                .decode_without_bom_handling(v)
                .0
                .to_string(),
        }
    }
}
impl<'a> fmt::Debug for Word<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_tuple("Word")
            .field(&format_args!("\"{}\"", self.to_string()))
            .finish()
    }
}

/// Word
///
/// ```abnf
///    word            =   atom / quoted-string
/// ```
pub fn word(input: &[u8]) -> IResult<&[u8], Word<'_>> {
    alt((
        map(quoted_string, Word::Quoted),
        map(encoded_word, Word::Encoded),
        map(atom, |a| Word::Atom(Cow::Borrowed(a))),
    ))(input)
}

#[derive(PartialEq, ToStatic)]
pub struct Phrase<'a>(pub Vec<Word<'a>>);

impl<'a> ToString for Phrase<'a> {
    fn to_string(&self) -> String {
        self.0
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<String>>()
            .join(" ")
    }
}
impl<'a> fmt::Debug for Phrase<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_tuple("Phrase")
            .field(&format_args!("\"{}\"", self.to_string()))
            .finish()
    }
}

/// Phrase
///
/// ```abnf
///    phrase          =   1*word / obs-phrase
/// ```
pub fn phrase(input: &[u8]) -> IResult<&[u8], Phrase<'_>> {
    let (input, phrase) = map(many1(word), Phrase)(input)?;
    Ok((input, phrase))
}

/// Compatible unstructured input
///
/// ```abnf
/// obs-utext       =   %d0 / obs-NO-WS-CTL / VCHAR
/// ```
fn is_unstructured(c: u8) -> bool {
    is_vchar(c) || is_obs_no_ws_ctl(c) || c == ascii::NULL
}

#[derive(PartialEq, Clone, ToStatic)]
pub enum UnstrToken<'a> {
    Encoded(encoding::EncodedWord<'a>),
    Plain(Cow<'a, [u8]>),
}
impl<'a> fmt::Debug for UnstrToken<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnstrToken::Encoded(e) => fmt.debug_tuple("Encoded").field(&e.to_string()).finish(),
            UnstrToken::Plain(s) => fmt
                .debug_tuple("Plain")
                .field(&String::from_utf8_lossy(&s))
                .finish(),
        }
    }
}

impl<'a> ToString for UnstrToken<'a> {
    fn to_string(&self) -> String {
        match self {
            UnstrToken::Encoded(e) => e.to_string(),
            UnstrToken::Plain(e) => encoding_rs::UTF_8
                .decode_without_bom_handling(e)
                .0
                .into_owned(),
        }
    }
}

#[derive(Debug, PartialEq, Clone, ToStatic)]
pub struct Unstructured<'a>(pub Vec<UnstrToken<'a>>);

impl<'a> ToString for Unstructured<'a> {
    fn to_string(&self) -> String {
        self.0
            .iter()
            .fold(
                (None, String::new()),
                |(prev_token, mut result), current_token| {
                    match (prev_token, current_token) {
                        (None, v) => result.push_str(v.to_string().as_ref()),
                        (Some(UnstrToken::Encoded(_)), UnstrToken::Encoded(v)) => {
                            result.push_str(v.to_string().as_ref())
                        }
                        (_, v) => {
                            result.push(' ');
                            result.push_str(v.to_string().as_ref())
                        }
                    };

                    (Some(current_token.clone()), result)
                },
            )
            .1
    }
}

/// Unstructured header field body
///
/// ```abnf
/// unstructured    =   (*([FWS] VCHAR_SEQ) *WSP) / obs-unstruct
/// ```
pub fn unstructured(input: &[u8]) -> IResult<&[u8], Unstructured<'_>> {
    let (input, r) = many0(preceded(
        opt(fws),
        alt((
            map(encoded_word, UnstrToken::Encoded),
            map(take_while1(is_unstructured), |p| {
                UnstrToken::Plain(Cow::Borrowed(p))
            }),
        )),
    ))(input)?;

    let (input, _) = space0(input)?;
    Ok((input, Unstructured(r)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_phrase() {
        assert_eq!(
            phrase(b"hello world").unwrap().1.to_string(),
            "hello world".to_string(),
        );
        assert_eq!(
            phrase(b"salut \"le\" monde").unwrap().1.to_string(),
            "salut le monde".to_string(),
        );

        let (rest, parsed) = phrase(b"fin\r\n du\r\nmonde").unwrap();
        assert_eq!(rest, &b"\r\nmonde"[..]);
        assert_eq!(parsed.to_string(), "fin du".to_string());
    }
}
