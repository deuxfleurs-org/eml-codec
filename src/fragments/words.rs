use crate::fragments::whitespace::cfws;
use nom::{
    bytes::complete::{tag, take_while1},
    combinator::{opt, recognize},
    multi::many0,
    sequence::{delimited, pair},
    IResult,
};

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
#[allow(dead_code)]
pub fn vchar_seq(input: &str) -> IResult<&str, &str> {
    take_while1(is_vchar)(input)
}

/// Atom allowed characters
fn is_atext(c: char) -> bool {
    c.is_ascii_alphanumeric() || "!#$%&'*+-/=?^_`{|}~".contains(c) || !c.is_ascii()
}

/// Atom
///
/// `[CFWS] 1*atext [CFWS]`
pub fn atom(input: &str) -> IResult<&str, &str> {
    delimited(opt(cfws), take_while1(is_atext), opt(cfws))(input)
}

/// dot-atom-text
///
/// `1*atext *("." 1*atext)`
pub fn dot_atom_text(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        take_while1(is_atext),
        many0(pair(tag("."), take_while1(is_atext))),
    ))(input)
}

/// dot-atom
///
/// `[CFWS] dot-atom-text [CFWS]`
pub fn dot_atom(input: &str) -> IResult<&str, &str> {
    delimited(opt(cfws), dot_atom_text, opt(cfws))(input)
}

#[allow(dead_code)]
pub fn is_special(c: char) -> bool {
    c == '('
        || c == ')'
        || c == '<'
        || c == '>'
        || c == '['
        || c == ']'
        || c == ':'
        || c == ';'
        || c == '@'
        || c == '\\'
        || c == ','
        || c == '.'
        || c == '"'
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
    fn test_atext() {
        assert!(is_atext('='));
        assert!(is_atext('5'));
        assert!(is_atext('Q'));
        assert!(!is_atext(' '));
        assert!(is_atext('É')); // support utf8
    }

    #[test]
    fn test_atom() {
        assert_eq!(
            atom("(skip)  imf_codec (hidden) aerogramme"),
            Ok(("aerogramme", "imf_codec"))
        );
    }

    #[test]
    fn test_dot_atom_text() {
        assert_eq!(
            dot_atom_text("quentin.dufour.io abcdef"),
            Ok((" abcdef", "quentin.dufour.io"))
        );
    }

    #[test]
    fn test_dot_atom() {
        assert_eq!(
            dot_atom("   (skip) quentin.dufour.io abcdef"),
            Ok(("abcdef", "quentin.dufour.io"))
        );
    }
}
