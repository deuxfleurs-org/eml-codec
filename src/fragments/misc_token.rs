use std::borrow::Cow;
use nom::{
    IResult,
    branch::alt,
    bytes::complete::{take_while1, tag},
    character::complete::space0,
    combinator::{into, opt},
    multi::{many0, many1, separated_list1},
    sequence::{pair, tuple},
};

use crate::fragments::lazy;
use crate::fragments::quoted::quoted_string;
use crate::fragments::whitespace::{fws, is_obs_no_ws_ctl};
use crate::fragments::words::{atom, is_vchar};
use crate::error::IMFError;

#[derive(Debug, PartialEq, Default)]
pub struct Unstructured(pub String);

#[derive(Debug, PartialEq, Default)]
pub struct PhraseList(pub Vec<String>);

impl<'a> TryFrom<&'a lazy::Unstructured<'a>> for Unstructured {
    type Error = IMFError<'a>;

    fn try_from(input: &'a lazy::Unstructured<'a>) -> Result<Self, Self::Error> {
        unstructured(input.0)
            .map(|(_, v)| Unstructured(v))
            .map_err(|e| IMFError::Unstructured(e))
    } 
}

impl<'a> TryFrom<&'a lazy::PhraseList<'a>> for PhraseList {
    type Error = IMFError<'a>;

    fn try_from(p: &'a lazy::PhraseList<'a>) -> Result<Self, Self::Error> {
        separated_list1(tag(","), phrase)(p.0)
            .map(|(_, q)| PhraseList(q))
            .map_err(|e| IMFError::PhraseList(e))
    }
}

/// Word
///
/// ```abnf
///    word            =   atom / quoted-string
/// ```
pub fn word(input: &str) -> IResult<&str, Cow<str>> {
    alt((into(quoted_string), into(atom)))(input)
}

/// Phrase
///
/// ```abnf
///    phrase          =   1*word / obs-phrase
/// ```
pub fn phrase(input: &str) -> IResult<&str, String> {
    let (input, words) = many1(word)(input)?;
    let phrase = words.join(" ");
    Ok((input, phrase))
}

/// Compatible unstructured input
///
/// ```abnf
/// obs-utext       =   %d0 / obs-NO-WS-CTL / VCHAR
/// ```
fn is_unstructured(c: char) -> bool {
    is_vchar(c) || is_obs_no_ws_ctl(c) || c == '\x00'
}

/// Unstructured header field body
///
/// ```abnf
/// unstructured    =   (*([FWS] VCHAR_SEQ) *WSP) / obs-unstruct
/// ```
pub fn unstructured(input: &str) -> IResult<&str, String> {
    let (input, r) = many0(tuple((opt(fws), take_while1(is_unstructured))))(input)?;
    let (input, _) = space0(input)?;

    // Try to optimize for the most common cases
    let body = match r.as_slice() {
        [(None, content)] => content.to_string(),
        [(Some(_), content)] => " ".to_string() + content,
        lines => lines.iter().fold(String::with_capacity(255), |acc, item| {
            let (may_ws, content) = item;
            match may_ws {
                Some(_) => acc + " " + content,
                None => acc + content,
            }
        }),
    };

    Ok((input, body))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_phrase() {
        assert_eq!(phrase("hello world"), Ok(("", "hello world".into())));
        assert_eq!(phrase("salut \"le\" monde"), Ok(("", "salut le monde".into())));
        assert_eq!(phrase("fin\r\n du\r\nmonde"), Ok(("\r\nmonde", "fin du".into())));
    }
}
