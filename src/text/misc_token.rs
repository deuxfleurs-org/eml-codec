use bounded_static::ToStatic;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_while1},
    character::complete::space0,
    combinator::{map, opt},
    multi::{many0, many1, separated_list1},
    sequence::pair,
    IResult,
    Parser,
};
use std::borrow::Cow;
use std::fmt;

use crate::text::{
    ascii,
    encoding::{self, encoded_word, encoded_word_plain},
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
    Atom(Cow<'a, [u8]>),
}

impl<'a> ToString for Word<'a> {
    fn to_string(&self) -> String {
        match self {
            Word::Quoted(v) => v.to_string(),
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
        map(atom, |a| Word::Atom(Cow::Borrowed(a))),
    ))(input)
}

#[derive(PartialEq, ToStatic)]
pub enum PhraseToken<'a> {
    Word(Word<'a>),
    Encoded(encoding::EncodedWord<'a>),
}
impl<'a> ToString for PhraseToken<'a> {
    fn to_string(&self) -> String {
        match self {
            PhraseToken::Word(w) => w.to_string(),
            PhraseToken::Encoded(e) => e.to_string(),
        }
    }
}
impl<'a> fmt::Debug for PhraseToken<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_tuple("PhraseToken")
            .field(&format_args!("\"{}\"", self.to_string()))
            .finish()
    }
}

pub fn phrase_token(input: &[u8]) -> IResult<&[u8], PhraseToken<'_>> {
    alt((
        // NOTE: we must try `encoded_word` first because encoded words
        // are also valid atoms
        map(encoded_word, PhraseToken::Encoded),
        map(word, PhraseToken::Word),
    ))(input)
}

// Must be a non-empty list
#[derive(PartialEq, ToStatic)]
pub struct Phrase<'a>(pub Vec<PhraseToken<'a>>);

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
///    phrase          =   1*(encoded-word / word) / obs-phrase
/// ```
///
/// (encoded-word comes from RFC2047)
///
/// TODO: obs-phrase
pub fn phrase(input: &[u8]) -> IResult<&[u8], Phrase<'_>> {
    let (input, phrase) = map(many1(phrase_token), Phrase)(input)?;
    Ok((input, phrase))
}

#[derive(Debug, PartialEq, Clone, ToStatic)]
pub struct UtextToken<'a> {
    txt: Cow<'a, [u8]>,
    obs: bool,
}

/// Compatible unstructured input
///
/// ```abnf
/// obs-utext       =   %d0 / obs-NO-WS-CTL / VCHAR
/// ```
///
/// The parser result records which parts of the input
/// were using the obsolete syntax (i.e. not VCHAR).
///
/// Parses a single run of either obsolete or non-obsolete
/// characters.
fn obs_utext_token<'a>(input: &'a [u8]) -> IResult<&'a [u8], UtextToken<'a>> {
    alt((
        take_while1(is_vchar)
            .map(|s| UtextToken { txt: Cow::Borrowed(s), obs: false }),
        take_while1(|c| is_obs_no_ws_ctl(c) || c == ascii::NULL)
            .map(|s| UtextToken { txt: Cow::Borrowed(s), obs: true }),
    ))(input)
}

#[derive(Debug, PartialEq, Clone, ToStatic)]
pub enum UnstrTxtKind {
    Txt,
    Obs,
    Fws,
}

#[derive(PartialEq, Clone, ToStatic)]
pub enum UnstrToken<'a> {
    Encoded(encoding::EncodedWord<'a>),
    Plain(Cow<'a, [u8]>, UnstrTxtKind),
}

impl<'a> UnstrToken<'a> {
    pub(crate) fn from_plain(s: &'a [u8], kind: UnstrTxtKind) -> Self {
        Self::Plain(Cow::Borrowed(s), kind)
    }

    fn from_utext(tok: UtextToken<'a>) -> Self {
        if tok.obs {
            Self::Plain(tok.txt, UnstrTxtKind::Obs)
        } else {
            Self::Plain(tok.txt, UnstrTxtKind::Txt)
        }
    }
}
impl<'a> fmt::Debug for UnstrToken<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnstrToken::Encoded(e) => fmt.debug_tuple("Encoded").field(&e.to_string()).finish(),
            UnstrToken::Plain(s, k) => fmt
                .debug_tuple("Plain")
                .field(&String::from_utf8_lossy(&s))
                .field(k)
                .finish(),
        }
    }
}

impl<'a> ToString for UnstrToken<'a> {
    fn to_string(&self) -> String {
        match self {
            UnstrToken::Encoded(e) => e.to_string(),
            // XXX discard obsolete tokens?
            UnstrToken::Plain(e, _) => encoding_rs::UTF_8
                .decode_without_bom_handling(&e)
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
/// obs_unstruct    =   *((*CR 1*(obs_utext / FWS)) / 1*LF) *CR   (cf errata)
/// ```
/// + RFC 2047 (MIME pt3) encoded words
///
/// We parse obs_unstruct, explicitly marking which part belong to the
/// obsolete syntax in the output AST.
// XXX the implementation below does not look spec compliant; idea below
// - perform pre-framing of headers first, cutting on CRLF (skipping CRLF WSP)
// - parse obs_unstruct (needs framing first, otherwise the greedy *CR at the
//   end would eat the final CRLF, erroring out when parsing a full header line
pub fn unstructured(input: &[u8]) -> IResult<&[u8], Unstructured<'_>> {
    let (input, r) = many0(pair(
        opt(fws),
        alt((
            map(encoded_word_plain, |w| vec![UnstrToken::Encoded(w)]),
            many1(map(obs_utext_token, UnstrToken::from_utext)),
        )),
    ))(input)?;
    let (input, wsp0) = space0(input)?;

    let mut tokens = vec![];
    for (fws_opt, toks) in r {
        if let Some(fws) = fws_opt {
            tokens.extend(fws.into_iter().map(|s| UnstrToken::from_plain(s, UnstrTxtKind::Fws)));
        }
        tokens.extend(toks);
    }
    if !wsp0.is_empty() {
        tokens.push(UnstrToken::from_plain(wsp0, UnstrTxtKind::Txt))
    }

    Ok((input, Unstructured(tokens)))
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
