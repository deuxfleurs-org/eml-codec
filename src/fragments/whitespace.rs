use nom::{
    IResult,
    branch::alt,
    bytes::complete::tag,
    character::complete::{crlf, satisfy, space0, space1},
    combinator::{recognize, opt},
    multi::{many0, many1},
    sequence::{pair, tuple},
};
use crate::fragments::quoted::quoted_pair;

// --- whitespaces and comments

// Note: WSP = SP / HTAB = %x20 / %x09
// nom::*::space0 = *WSP
// nom::*::space1 = 1*WSP

/// Permissive CRLF
///
/// Theoretically, all lines must end with \r\n
/// but some mail servers like Dovecot support malformated emails,
/// for example with only \n eol. It works because
/// \r or \n is allowed nowhere else, so we also add this support.
pub fn perm_crlf(input: &str) -> IResult<&str, &str> {
    alt((crlf, tag("\r"), tag("\n")))(input)
}

/// Permissive foldable white space
///
/// Folding white space are used for long headers splitted on multiple lines.
/// The obsolete syntax allowes multiple lines without content; implemented for compatibility
/// reasons
pub fn fws(input: &str) -> IResult<&str, char> {
    let (input, _) = alt((recognize(many1(fold_marker)), space1))(input)?;
    Ok((input, ' '))
}
fn fold_marker(input: &str) -> IResult<&str, &str> {
   let (input, _) = space0(input)?;
   let (input, _) = perm_crlf(input)?;
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
pub fn cfws(input: &str) -> IResult<&str, &str> {
    alt((recognize(comments), recognize(fws)))(input)
}

pub fn comments(input: &str) -> IResult<&str, ()> {
    let (input, _) = many1(tuple((opt(fws), comment)))(input)?;
    let (input, _) = opt(fws)(input)?;
    Ok((input, ()))
}

pub fn comment(input: &str) -> IResult<&str, ()> {
    let (input, _) = tag("(")(input)?;
    let (input, _) = many0(tuple((opt(fws), ccontent)))(input)?;
    let (input, _) = opt(fws)(input)?;
    let (input, _) = tag(")")(input)?;
    Ok((input, ()))
}

pub fn ccontent(input: &str) -> IResult<&str, &str> {
   alt((recognize(ctext), recognize(quoted_pair), recognize(comment)))(input) 
}

pub fn ctext(input: &str) -> IResult<&str, char> {
    satisfy(is_ctext)(input)
}

/// Check if it's a comment text character
///
/// ```abnf
///   ctext           =   %d33-39 /          ; Printable US-ASCII
///                       %d42-91 /          ;  characters not including
///                       %d93-126 /         ;  "(", ")", or "\"
///                       obs-ctext
///```
pub fn is_restr_ctext(c: char) -> bool {
    (c >= '\x21' && c <= '\x27') || (c >= '\x2A' && c <= '\x5B') || (c >= '\x5D' && c <= '\x7E') || !c.is_ascii()
}

pub fn is_ctext(c: char) -> bool {
    is_restr_ctext(c) || is_obs_no_ws_ctl(c)
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
pub fn is_obs_no_ws_ctl(c: char) -> bool {
    (c >= '\x01' && c <= '\x08') || c == '\x0b' || c == '\x0b' || (c >= '\x0e' && c<= '\x1f') || c == '\x7F'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perm_crlf() {
        assert_eq!(perm_crlf("\rworld"), Ok(("world", "\r")));
        assert_eq!(perm_crlf("\r\nworld"), Ok(("world", "\r\n")));
        assert_eq!(perm_crlf("\nworld"), Ok(("world", "\n")));
    }

    #[test]
    fn test_fws() {
        assert_eq!(fws("\r\n world"), Ok(("world", ' ')));
        assert_eq!(fws(" \r\n \r\n world"), Ok(("world", ' ')));
        assert_eq!(fws(" world"), Ok(("world", ' ')));
        assert!(fws("\r\nFrom: test").is_err());
    }

    #[test]
    fn test_cfws() {
        assert_eq!(cfws("(A nice \\) chap) <pete(his account)@silly.test(his host)>"), Ok(("<pete(his account)@silly.test(his host)>", "(A nice \\) chap) ")));
        assert_eq!(cfws("(Chris's host.)public.example>,"), Ok(("public.example>,", "(Chris's host.)")));
        assert_eq!(cfws("(double (comment) is fun) wouch"), Ok(("wouch", "(double (comment) is fun) ")));
    }
}
