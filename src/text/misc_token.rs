use nom::{
    branch::alt,
    bytes::complete::take_while1,
    character::complete::space0,
    combinator::{map, opt},
    multi::{many0, many1},
    sequence::{preceded},
    IResult,
};

use crate::text::{
    quoted::quoted_string,
    whitespace::{fws, is_obs_no_ws_ctl},
    words::{atom, is_vchar},
    encoding::{self, encoded_word},
    buffer,
    ascii,
};

#[derive(Debug, PartialEq, Default)]
pub struct PhraseList(pub Vec<String>);

/*
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
}*/

pub enum Word<'a> {
    Quoted(buffer::Text<'a>),
    Encoded(encoding::EncodedWord<'a>),
    Atom(&'a [u8]),
}
impl<'a> Word<'a> {
    pub fn to_string(&self) -> String {
        match self {
            Word::Quoted(v) => v.to_string(),
            Word::Encoded(v) => v.to_string(),
            Word::Atom(v) => encoding_rs::UTF_8.decode_without_bom_handling(v).0.to_string(),
        }
    }
}

/// Word
///
/// ```abnf
///    word            =   atom / quoted-string
/// ```
pub fn word(input: &[u8]) -> IResult<&[u8], Word> {
    alt((
        map(quoted_string, |v| Word::Quoted(v)), 
        map(encoded_word, |v| Word::Encoded(v)),
        map(atom, |v| Word::Atom(v))
    ))(input)
}

pub struct Phrase<'a>(pub Vec<Word<'a>>);
impl<'a> Phrase<'a> {
    pub fn to_string(&self) -> String {
        self.0.iter().map(|v| v.to_string()).collect::<Vec<String>>().join(" ")
    }
}

/// Phrase
///
/// ```abnf
///    phrase          =   1*word / obs-phrase
/// ```
pub fn phrase(input: &[u8]) -> IResult<&[u8], Phrase> {
    let (input, phrase) = map(many1(word), |v| Phrase(v))(input)?;
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

pub enum UnstrToken<'a> {
    Init,
    Encoded(encoding::EncodedWord<'a>),
    Plain(&'a [u8]),
}
impl<'a> UnstrToken<'a> {
    pub fn to_string(&self) -> String {
        match self {
            UnstrToken::Init => "".into(),
            UnstrToken::Encoded(e) => e.to_string(),
            UnstrToken::Plain(e) => encoding_rs::UTF_8.decode_without_bom_handling(e).0.into_owned(),
        }
    }
}

pub struct Unstructured<'a>(pub Vec<UnstrToken<'a>>);
impl<'a> Unstructured<'a> {
    pub fn to_string(&self) -> String {
        self.0.iter().fold(
            (&UnstrToken::Init, String::new()),
            |(prev_token, mut result), current_token| {
                match (prev_token, current_token) {
                    (UnstrToken::Init, v) => result.push_str(v.to_string().as_ref()),
                    (UnstrToken::Encoded(_), UnstrToken::Encoded(v)) => result.push_str(v.to_string().as_ref()),
                    (_, v) => {
                        result.push(' ');
                        result.push_str(v.to_string().as_ref())
                    },
                };

                (current_token, result)
            }
        ).1
    }
}

/// Unstructured header field body
///
/// ```abnf
/// unstructured    =   (*([FWS] VCHAR_SEQ) *WSP) / obs-unstruct
/// ```
pub fn unstructured(input: &[u8]) -> IResult<&[u8], Unstructured> {
    let (input, r) = many0(preceded(opt(fws), alt((
                        map(encoded_word, |v| UnstrToken::Encoded(v)), 
                        map(take_while1(is_unstructured), |v| UnstrToken::Plain(v)),
                    ))))(input)?;

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
