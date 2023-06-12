use nom::{
    IResult,
    branch::alt,
    bytes::complete::{tag, take_while1},
    character::complete::{crlf, satisfy, space0, space1},
    combinator::{recognize, opt},
    multi::{many0, many1},
    sequence::{preceded, terminated, tuple},
};

/// Lexical tokens
///
/// Approx. maps to section 3.2 of the RFC 
/// https://www.rfc-editor.org/rfc/rfc5322#section-3.2
/// Also https://datatracker.ietf.org/doc/html/rfc6532

/// Permissive CRLF
///
/// Theoretically, all lines must end with \r\n
/// but some mail servers like Dovecot support malformated emails,
/// for example with only \n eol. It works because
/// \r or \n is allowed nowhere else, so we also add this support.
pub fn perm_crlf(input: &str) -> IResult<&str, &str> {
    alt((crlf, tag("\r"), tag("\n")))(input)
}

// Note: WSP = SP / HTAB = %x20 / %x09
// nom::*::space0 = *WSP
// nom::*::space1 = 1*WSP

/// Quoted pair
///
/// ```abnf
///    quoted-pair     =   ("\" (VCHAR / WSP)) / obs-qp
/// ```
pub fn quoted_pair(input: &str) -> IResult<&str, char> {
    preceded(tag("\\"), satisfy(|c| is_vchar(c) || c == '\t' || c == ' '))(input)
}

/// Permissive foldable white space
///
/// Folding white space are used for long headers splitted on multiple lines.
/// The obsolete syntax allowes multiple lines without content; implemented for compatibility
/// reasons
pub fn perm_fws(input: &str) -> IResult<&str, &str> {
    alt((recognize(many1(fold_marker)), space1))(input)
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
    alt((perm_fws, recognize(comments)))(input)
}

pub fn comments(input: &str) -> IResult<&str, ()> {
    let (input, _) = many1(tuple((opt(perm_fws), comment)))(input)?;
    let (input, _) = opt(perm_fws)(input)?;
    Ok((input, ()))
}

pub fn comment(input: &str) -> IResult<&str, ()> {
    let (input, _) = tag("(")(input)?;
    let (input, _) = many0(tuple((opt(perm_fws), ccontent)))(input)?;
    let (input, _) = opt(perm_fws)(input)?;
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
pub fn is_ctext(c: char) -> bool {
    (c >= '\x21' && c <= '\x27') || (c >= '\x2A' && c <= '\x5B') || (c >= '\x5D' && c <= '\x7E') || !c.is_ascii()
}

/// VCHAR definition
pub fn is_vchar(c: char) -> bool {
  (c >= '\x21' && c <= '\x7E') || !c.is_ascii()
}

/// Sequence of visible chars with the UTF-8 extension
///
/// ```abnf
/// VCHAR   =  %x21-7E
///            ; visible (printing) characters
/// VCHAR   =/  UTF8-non-ascii
/// SEQ     = 1*VCHAR
///```
pub fn vchar_seq(input: &str) -> IResult<&str, &str> {
   take_while1(is_vchar)(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nom;

    #[test]
    fn test_vchar_seq() {
        assert_eq!(vchar_seq("hello world"), Ok((" world", "hello")));
        assert_eq!(vchar_seq("hello👋 world"), Ok((" world", "hello👋")));
    }

    #[test]
    fn test_perm_crlf() {
        assert_eq!(perm_crlf("\rworld"), Ok(("world", "\r")));
        assert_eq!(perm_crlf("\r\nworld"), Ok(("world", "\r\n")));
        assert_eq!(perm_crlf("\nworld"), Ok(("world", "\n")));
    }

    #[test]
    fn test_perm_fws() {
        assert_eq!(perm_fws("\r\n world"), Ok(("world", "\r\n ")));
        assert_eq!(perm_fws(" \r\n \r\n world"), Ok(("world", " \r\n \r\n ")));
        assert_eq!(perm_fws(" world"), Ok(("world", " ")));
        assert!(perm_fws("\r\nFrom: test").is_err());
    }

    #[test]
    fn test_cfws() {
        assert_eq!(cfws("(A nice \\) chap) <pete(his account)@silly.test(his host)>"), Ok(("<pete(his account)@silly.test(his host)>", "(A nice \\) chap) ")));
    }
}
