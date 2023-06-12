use nom::{
    IResult,
    branch::alt,
    bytes::complete::{tag, take_while1},
    character::complete::{crlf, satisfy, space0, space1},
    combinator::{recognize, opt},
    multi::{many0, many1},
    sequence::{delimited, pair, preceded, terminated, tuple},
};

/// Lexical tokens
///
/// Approx. maps to section 3.2 of the RFC 
/// https://www.rfc-editor.org/rfc/rfc5322#section-3.2
/// Also https://datatracker.ietf.org/doc/html/rfc6532

// quoted characters and strings

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
pub fn is_ctext(c: char) -> bool {
    (c >= '\x21' && c <= '\x27') || (c >= '\x2A' && c <= '\x5B') || (c >= '\x5D' && c <= '\x7E') || !c.is_ascii()
}

// atoms, words, phrases, vchar

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

/// Atom allowed characters
fn is_atext(c: char) -> bool {
    c.is_ascii_alphanumeric() || "!#$%&'*+-/=?^_`{|}~".contains(c)
}

/// Atom 
///
/// `[CFWS] 1*atext [CFWS]`
fn atom(input: &str) -> IResult<&str, &str> {
    delimited(opt(cfws), take_while1(is_atext), opt(cfws))(input)
}

/// dot-atom-text
///
/// `1*atext *("." 1*atext)`
fn dot_atom_text(input: &str) -> IResult<&str, &str> {
    recognize(pair(take_while1(is_atext), many0(pair(tag("."), take_while1(is_atext)))))(input)
}

/// dot-atom
///
/// `[CFWS] dot-atom-text [CFWS]`
fn dot_atom(input: &str) -> IResult<&str, &str> {
    delimited(opt(cfws), dot_atom_text, opt(cfws))(input)
}


#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_atext() {
        assert!(is_atext('='));
        assert!(is_atext('5'));
        assert!(is_atext('Q'));
        assert!(!is_atext(' '));
        assert!(!is_atext('É'));
    }

    #[test]
    fn test_atom() {
        assert_eq!(atom("(skip)  imf_codec (hidden) aerogramme"), Ok(("aerogramme", "imf_codec")));
    }

    #[test]
    fn test_dot_atom_text() {
        assert_eq!(dot_atom_text("quentin.dufour.io abcdef"), Ok((" abcdef", "quentin.dufour.io")));
    }

    #[test]
    fn test_dot_atom() {
        assert_eq!(dot_atom("   (skip) quentin.dufour.io abcdef"), Ok(("abcdef", "quentin.dufour.io")));
    }

    #[test]
    fn test_quoted_string() {
        assert_eq!(quoted_string(" \"hello\\\"world\" "), Ok(("", "hello\"world".to_string())));
        assert_eq!(quoted_string("\"hello\r\n world\""), Ok(("", "hello world".to_string())));
    }
}
