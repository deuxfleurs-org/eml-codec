use nom::{
    IResult,
    branch::alt,
    bytes::complete::tag,
    character::complete::satisfy,
    combinator::opt,
    multi::many0,
    sequence::{pair, preceded},
};

use crate::words::is_vchar;
use crate::whitespace::{fws, cfws};

/// Quoted pair
///
/// ```abnf
///    quoted-pair     =   ("\" (VCHAR / WSP)) / obs-qp
/// ```
pub fn quoted_pair(input: &str) -> IResult<&str, char> {
    preceded(tag("\\"), satisfy(|c| is_vchar(c) || c == '\t' || c == ' '))(input)
}

/// Allowed characters in quote
///
/// ```abnf
///   qtext           =   %d33 /             ; Printable US-ASCII
///                       %d35-91 /          ;  characters not including
///                       %d93-126 /         ;  "\" or the quote character
///                       obs-qtext
/// ```
fn is_qtext(c: char) -> bool {
    c == '\x21' || (c >= '\x23' && c <= '\x5B') || (c >= '\x5D' && c <= '\x7E')
}

/// Quoted pair content
///
/// ```abnf
///   qcontent        =   qtext / quoted-pair
/// ```
fn qcontent(input: &str) -> IResult<&str, char> {
    alt((satisfy(is_qtext), quoted_pair))(input)
}

/// Quoted string
///
/// ```abnf
/// quoted-string   =   [CFWS]
///                     DQUOTE *([FWS] qcontent) [FWS] DQUOTE
///                     [CFWS]
/// ```
pub fn quoted_string(input: &str) -> IResult<&str, String> {
  let (input, _) = opt(cfws)(input)?;
  let (input, _) = tag("\"")(input)?;
  let (input, content) = many0(pair(opt(fws), qcontent))(input)?;

  // Rebuild string
  let mut qstring = content.iter().fold(
    String::with_capacity(16), 
    |mut acc, (maybe_wsp, c)| {
      if let Some(wsp) = maybe_wsp {
        acc.push(*wsp);
      }
      acc.push(*c);
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
        assert_eq!(quoted_string(" \"hello\\\"world\" "), Ok(("", "hello\"world".to_string())));
        assert_eq!(quoted_string("\"hello\r\n world\""), Ok(("", "hello world".to_string())));
    }
}
