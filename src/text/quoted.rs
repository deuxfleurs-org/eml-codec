use nom::{
    branch::alt,
    bytes::complete::{tag, take, take_while1},
    combinator::opt,
    multi::many0,
    sequence::{pair, preceded},
    IResult,
};

use crate::text::ascii;
use crate::text::whitespace::{cfws, fws, is_obs_no_ws_ctl};

#[derive(Debug, PartialEq, Default, Clone)]
pub struct QuotedString<'a>(pub Vec<&'a [u8]>);

impl<'a> QuotedString<'a> {
    pub fn push(&mut self, e: &'a [u8]) {
        self.0.push(e)
    }

    pub fn to_string(&self) -> String {
        let enc = encoding_rs::UTF_8;
        let size = self.0.iter().fold(0, |acc, v| acc + v.len());

        self.0
            .iter()
            .fold(String::with_capacity(size), |mut acc, v| {
                let (content, _) = enc.decode_without_bom_handling(v);
                acc.push_str(content.as_ref());
                acc
            })
    }
}

/// Quoted pair
///
/// ```abnf
///    quoted-pair     =   ("\" (VCHAR / WSP)) / obs-qp
///    obs-qp          =   "\" (%d0 / obs-NO-WS-CTL / LF / CR)
/// ```
pub fn quoted_pair(input: &[u8]) -> IResult<&[u8], &[u8]> {
    preceded(tag(&[ascii::BACKSLASH]), take(1usize))(input)
}

/// Allowed characters in quote
///
/// ```abnf
///   qtext           =   %d33 /             ; Printable US-ASCII
///                       %d35-91 /          ;  characters not including
///                       %d93-126 /         ;  "\" or the quote character
///                       obs-qtext
/// ```
fn is_restr_qtext(c: u8) -> bool {
    c == ascii::EXCLAMATION
        || (ascii::NUM..=ascii::LEFT_BRACKET).contains(&c)
        || (ascii::RIGHT_BRACKET..=ascii::TILDE).contains(&c)
}

fn is_qtext(c: u8) -> bool {
    is_restr_qtext(c) || is_obs_no_ws_ctl(c)
}

/// Quoted pair content
///
/// ```abnf
///   qcontent        =   qtext / quoted-pair
/// ```
fn qcontent(input: &[u8]) -> IResult<&[u8], &[u8]> {
    alt((take_while1(is_qtext), quoted_pair))(input)
}

/// Quoted string
///
/// ```abnf
/// quoted-string   =   [CFWS]
///                     DQUOTE *([FWS] qcontent) [FWS] DQUOTE
///                     [CFWS]
/// ```
pub fn quoted_string(input: &[u8]) -> IResult<&[u8], QuotedString> {
    let (input, _) = opt(cfws)(input)?;
    let (input, _) = tag("\"")(input)?;
    let (input, content) = many0(pair(opt(fws), qcontent))(input)?;

    // Rebuild string
    let mut qstring = content
        .iter()
        .fold(QuotedString::default(), |mut acc, (maybe_wsp, c)| {
            if maybe_wsp.is_some() {
                acc.push(&[ascii::SP]);
            }
            acc.push(c);
            acc
        });

    let (input, maybe_wsp) = opt(fws)(input)?;
    if maybe_wsp.is_some() {
        qstring.push(&[ascii::SP]);
    }

    let (input, _) = tag("\"")(input)?;
    let (input, _) = opt(cfws)(input)?;
    Ok((input, qstring))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quoted_string_parser() {
        assert_eq!(
            quoted_string(b" \"hello\\\"world\" ").unwrap().1,
            QuotedString(vec![b"hello", &[ascii::DQUOTE], b"world"])
        );

        assert_eq!(
            quoted_string(b"\"hello\r\n world\""),
            Ok((
                &b""[..],
                QuotedString(vec![b"hello", &[ascii::SP], b"world"])
            )),
        );
    }

    use crate::text::ascii;

    #[test]
    fn test_quoted_string_object() {
        assert_eq!(
            QuotedString(vec![b"hello", &[ascii::SP], b"world"]).to_string(),
            "hello world".to_string(),
        );
    }
}
