use nom::{
    IResult,
    branch::alt,
    bytes::complete::{tag, take_while1},
    character::complete::{crlf, space0, space1},
    combinator::opt,
    sequence::terminated,
};

/// ABNF rfc5234

/// Permissive CRLF
///
/// Theoretically, all lines must end with \r\n
/// but mail servers support malformated emails,
/// for example with only \n eol. It works because
/// \r\n is allowed nowhere else, so we also add this support.
pub fn perm_crlf(input: &str) -> IResult<&str, &str> {
    alt((crlf, tag("\r"), tag("\n")))(input)
}

// Note: WSP = SP / HTAB = %x20 / %x09
// nom::*::space0 = *WSP
// nom::*::space1 = 1*WSP

/// Parse a folding white space
///
/// Folding white space are used for long headers splitted on multiple lines
///
/// ```abnf
/// FWS             =   ([*WSP CRLF] 1*WSP) /  obs-FWS
/// obs-FWS         =   1*WSP *(CRLF 1*WSP)
/// ```
pub fn fws(input: &str) -> IResult<&str, &str> {
    let (input, _) = opt(terminated(space0, perm_crlf))(input)?;
    // @FIXME: not implemented obs-FWS
    space1(input)
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
   take_while1(|c: char| (c >= '\x21' && c <= '\x7E') || !c.is_ascii())(input)
}
