use crate::text::ascii;
use crate::text::encoding::encoded_word;
use crate::text::quoted::quoted_pair;
use nom::{
    branch::alt,
    bytes::complete::{is_not, tag, take_while1},
    character::complete::{space0, space1},
    combinator::{opt, recognize},
    multi::{many0, many1},
    sequence::{pair, terminated, tuple},
    IResult,
};

/// Whitespace (space, new line, tab) content and
/// delimited content (eg. comment, line, sections, etc.)

/// Obsolete/Compatible CRLF
///
/// Theoretically, all lines must end with \r\n
/// but some mail servers like Dovecot support malformated emails,
/// for example with only \n eol. It works because
/// \r or \n is allowed nowhere else, so we also add this support.

pub fn obs_crlf(input: &[u8]) -> IResult<&[u8], &[u8]> {
    alt((
        tag(ascii::CRLF),
        tag(ascii::CRCRLF),
        tag(&[ascii::CR]),
        tag(&[ascii::LF]),
    ))(input)
}

/// ```abnf
/// fold_line = any *(1*(crlf WS) any) crlf
/// ```
pub fn foldable_line(input: &[u8]) -> IResult<&[u8], &[u8]> {
    terminated(
        recognize(tuple((
            is_not(ascii::CRLF),
            many0(pair(many1(pair(obs_crlf, space1)), is_not(ascii::CRLF))),
        ))),
        obs_crlf,
    )(input)
}

// --- whitespaces and comments

// Note: WSP = SP / HTAB = %x20 / %x09
// nom::*::space0 = *WSP
// nom::*::space1 = 1*WSP

/// Permissive foldable white space
///
/// Folding white space are used for long headers splitted on multiple lines.
/// The obsolete syntax allowes multiple lines without content; implemented for compatibility
/// reasons
pub fn fws(input: &[u8]) -> IResult<&[u8], u8> {
    let (input, _) = alt((recognize(many1(fold_marker)), space1))(input)?;
    Ok((input, ascii::SP))
}
fn fold_marker(input: &[u8]) -> IResult<&[u8], &[u8]> {
    let (input, _) = space0(input)?;
    let (input, _) = obs_crlf(input)?;
    space1(input)
}

/// Folding White Space with Comment
///
/// Note: we drop the comments for now...  
///
/// ```abnf
///   ctext           =   %d33-39 /          ; Printable US-ASCII
///                       %d42-91 /          ;  characters not including
///                       %d93-126 /         ;  "(", ")", or "\"
///                       obs-ctext
///
///   ccontent        =   ctext / quoted-pair / comment
///
///   comment         =   "(" *([FWS] ccontent) [FWS] ")"
///
///   CFWS            =   (1*([FWS] comment) [FWS]) / FWS
/// ```
pub fn cfws(input: &[u8]) -> IResult<&[u8], &[u8]> {
    alt((recognize(comments), recognize(fws)))(input)
}

pub fn comments(input: &[u8]) -> IResult<&[u8], ()> {
    let (input, _) = many1(tuple((opt(fws), comment)))(input)?;
    let (input, _) = opt(fws)(input)?;
    Ok((input, ()))
}

pub fn comment(input: &[u8]) -> IResult<&[u8], ()> {
    let (input, _) = tag("(")(input)?;
    let (input, _) = many0(tuple((opt(fws), ccontent)))(input)?;
    let (input, _) = opt(fws)(input)?;
    let (input, _) = tag(")")(input)?;
    Ok((input, ()))
}

pub fn ccontent(input: &[u8]) -> IResult<&[u8], &[u8]> {
    alt((
        ctext,
        recognize(quoted_pair),
        recognize(encoded_word),
        recognize(comment),
    ))(input)
}

pub fn ctext(input: &[u8]) -> IResult<&[u8], &[u8]> {
    take_while1(is_ctext)(input)
}

pub fn is_ctext(c: u8) -> bool {
    is_restr_ctext(c) || is_obs_no_ws_ctl(c)
}

/// Check if it's a comment text character
///
/// ```abnf
///   ctext           =   %d33-39 /          ; Printable US-ASCII
///                       %d42-91 /          ;  characters not including
///                       %d93-126 /         ;  "(", ")", or "\"
///                       obs-ctext
///```
pub fn is_restr_ctext(c: u8) -> bool {
    (ascii::EXCLAMATION..=ascii::SQUOTE).contains(&c)
        || (ascii::ASTERISK..=ascii::LEFT_BRACKET).contains(&c)
        || (ascii::RIGHT_BRACKET..=ascii::TILDE).contains(&c)
}

/// US ASCII control characters without effect
///
/// ```abnf
///   obs-NO-WS-CTL   =   %d1-8 /            ; US-ASCII control
///                       %d11 /             ;  characters that do not
///                       %d12 /             ;  include the carriage
///                       %d14-31 /          ;  return, line feed, and
///                       %d127              ;  white space characters
/// ```
pub fn is_obs_no_ws_ctl(c: u8) -> bool {
    (ascii::SOH..=ascii::BS).contains(&c)
        || c == ascii::VT
        || c == ascii::FF
        || (ascii::SO..=ascii::US).contains(&c)
        || c == ascii::DEL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_obs_crlf() {
        assert_eq!(obs_crlf(b"\rworld"), Ok((&b"world"[..], &b"\r"[..])));
        assert_eq!(obs_crlf(b"\r\nworld"), Ok((&b"world"[..], &b"\r\n"[..])));
        assert_eq!(obs_crlf(b"\nworld"), Ok((&b"world"[..], &b"\n"[..])));
    }

    #[test]
    fn test_fws() {
        assert_eq!(fws(b"\r\n world"), Ok((&b"world"[..], ascii::SP)));
        assert_eq!(fws(b" \r\n \r\n world"), Ok((&b"world"[..], ascii::SP)));
        assert_eq!(fws(b" world"), Ok((&b"world"[..], ascii::SP)));
        assert!(fws(b"\r\nFrom: test").is_err());
    }

    #[test]
    fn test_cfws() {
        assert_eq!(
            cfws(b"(A nice \\) chap) <pete(his account)@silly.test(his host)>"),
            Ok((
                &b"<pete(his account)@silly.test(his host)>"[..],
                &b"(A nice \\) chap) "[..]
            ))
        );
        assert_eq!(
            cfws(b"(Chris's host.)public.example>,"),
            Ok((&b"public.example>,"[..], &b"(Chris's host.)"[..]))
        );
        assert_eq!(
            cfws(b"(double (comment) is fun) wouch"),
            Ok((&b"wouch"[..], &b"(double (comment) is fun) "[..]))
        );
    }

    #[test]
    fn test_cfws_encoded_word() {
        assert_eq!(
            cfws(b"(=?US-ASCII?Q?Keith_Moore?=)"),
            Ok((&b""[..], &b"(=?US-ASCII?Q?Keith_Moore?=)"[..])),
        );
    }
}
