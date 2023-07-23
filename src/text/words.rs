use crate::text::ascii;
use crate::text::whitespace::cfws;
use nom::{
    bytes::complete::{tag, take_while1},
    character::is_alphanumeric,
    combinator::{opt, recognize},
    multi::many0,
    sequence::{delimited, pair},
    IResult,
};

pub fn is_vchar(c: u8) -> bool {
    c >= ascii::EXCLAMATION && c <= ascii::TILDE
}

/// MIME Token allowed characters
///
/// forbidden: ()<>@,;:\"/[]?=
fn is_mime_atom_text(c: u8) -> bool {
    is_alphanumeric(c)
        || c == ascii::EXCLAMATION
        || c == ascii::NUM
        || c == ascii::DOLLAR
        || c == ascii::PERCENT
        || c == ascii::AMPERSAND
        || c == ascii::SQUOTE
        || c == ascii::ASTERISK
        || c == ascii::PLUS
        || c == ascii::MINUS
        || c == ascii::PERIOD
        || c == ascii::CARRET
        || c == ascii::UNDERSCORE
        || c == ascii::GRAVE
        || c == ascii::LEFT_CURLY
        || c == ascii::PIPE
        || c == ascii::RIGHT_CURLY
        || c == ascii::TILDE
}

/// MIME Token
///
/// `[CFWS] 1*token_text [CFWS]`
pub fn mime_atom(input: &[u8]) -> IResult<&[u8], &[u8]> {
    delimited(opt(cfws), take_while1(is_mime_atom_text), opt(cfws))(input)
}

/// Atom allowed characters
///
/// authorized: !#$%&'*+-/=?^_`{|}~
fn is_atext(c: u8) -> bool {
    is_alphanumeric(c)
        || c == ascii::EXCLAMATION
        || c == ascii::NUM
        || c == ascii::DOLLAR
        || c == ascii::PERCENT
        || c == ascii::AMPERSAND
        || c == ascii::SQUOTE
        || c == ascii::ASTERISK
        || c == ascii::PLUS
        || c == ascii::MINUS
        || c == ascii::SLASH
        || c == ascii::EQ
        || c == ascii::QUESTION
        || c == ascii::CARRET
        || c == ascii::UNDERSCORE
        || c == ascii::GRAVE
        || c == ascii::LEFT_CURLY
        || c == ascii::PIPE
        || c == ascii::RIGHT_CURLY
        || c == ascii::TILDE
}

/// Atom
///
/// `[CFWS] 1*atext [CFWS]`
pub fn atom(input: &[u8]) -> IResult<&[u8], &[u8]> {
    delimited(opt(cfws), take_while1(is_atext), opt(cfws))(input)
}

/// dot-atom-text
///
/// `1*atext *("." 1*atext)`
pub fn dot_atom_text(input: &[u8]) -> IResult<&[u8], &[u8]> {
    recognize(pair(
        take_while1(is_atext),
        many0(pair(tag("."), take_while1(is_atext))),
    ))(input)
}

/// dot-atom
///
/// `[CFWS] dot-atom-text [CFWS]`
pub fn dot_atom(input: &[u8]) -> IResult<&[u8], &[u8]> {
    delimited(opt(cfws), dot_atom_text, opt(cfws))(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atext() {
        assert!(is_atext('=' as u8));
        assert!(is_atext('5' as u8));
        assert!(is_atext('Q' as u8));
        assert!(!is_atext(' ' as u8));
        //assert!(is_atext('É')); // support utf8
    }

    #[test]
    fn test_atom() {
        assert_eq!(
            atom(b"(skip)  imf_codec (hidden) aerogramme"),
            Ok((&b"aerogramme"[..], &b"imf_codec"[..]))
        );
    }

    #[test]
    fn test_dot_atom_text() {
        assert_eq!(
            dot_atom_text(b"quentin.dufour.io abcdef"),
            Ok((&b" abcdef"[..], &b"quentin.dufour.io"[..]))
        );
    }

    #[test]
    fn test_dot_atom() {
        assert_eq!(
            dot_atom(b"   (skip) quentin.dufour.io abcdef"),
            Ok((&b"abcdef"[..], &b"quentin.dufour.io"[..]))
        );
    }
}
