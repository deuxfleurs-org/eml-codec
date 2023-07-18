use nom::{
    branch::alt,
    bytes::complete::{take_while1, tag},
    character::complete::anychar,
    combinator::{recognize, opt},
    multi::many0,
    sequence::{pair, preceded},
    IResult,
};

use crate::text::whitespace::{cfws, fws, is_obs_no_ws_ctl};
use crate::text::ascii;
use crate::text::buffer;

/// Quoted pair
///
/// ```abnf
///    quoted-pair     =   ("\" (VCHAR / WSP)) / obs-qp
///    obs-qp          =   "\" (%d0 / obs-NO-WS-CTL / LF / CR)
/// ```
pub fn quoted_pair(input: &[u8]) -> IResult<&[u8], u8> {
    preceded(tag(&[ascii::SLASH]), anychar)(input)
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
    c == ascii::EXCLAMATION || (c >= ascii::NUM && c <= ascii::LEFT_BRACKET) || (c >= ascii::RIGHT_BRACKET && c <= ascii::TILDE)
}

fn is_qtext(c: u8) -> bool {
    is_restr_qtext(c) || is_obs_no_ws_ctl(c)
}

/// Quoted pair content
///
/// ```abnf
///   qcontent        =   qtext / quoted-pair
/// ```
fn qcontent(input: &u8) -> IResult<&[u8], &[u8]> {
    alt((take_while1(is_qtext), recognize(quoted_pair)))(input)
}

/// Quoted string
///
/// ```abnf
/// quoted-string   =   [CFWS]
///                     DQUOTE *([FWS] qcontent) [FWS] DQUOTE
///                     [CFWS]
/// ```
pub fn quoted_string(input: &[u8]) -> IResult<&[u8], buffer::Text> {
    let (input, _) = opt(cfws)(input)?;
    let (input, _) = tag("\"")(input)?;
    let (input, content) = many0(pair(opt(fws), qcontent))(input)?;

    // Rebuild string
    let mut qstring = content
        .iter()
        .fold(buffer::Text::default(), |mut acc, (maybe_wsp, c)| {
            if let Some(wsp) = maybe_wsp {
                acc.push(&[ascii::SP]);
            }
            acc.push(c);
            acc
        });

    let (input, maybe_wsp) = opt(fws)(input)?;
    if let Some(wsp) = maybe_wsp {
        qstring.push(wsp);
    }

    let (input, _) = tag("\"")(input)?;
    let (input, _) = opt(cfws)(input)?;
    Ok((input, qstring))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quoted_string() {
        let mut text = buffer::Text::default();
        text.push(b"hello");
        text.push(&[ascii::DQUOTE]);
        text.push(b"world");
        assert_eq!(
            quoted_string(b" \"hello\\\"world\" "),
            Ok(("", text))
        );

        let mut text = buffer::Text::default();
        text.push(b"hello");
        text.push(&[ascii::SP]);
        text.push(b"world");
        assert_eq!(
            quoted_string(b"\"hello\r\n world\""),
            Ok(("", text))
        );
    }
}
