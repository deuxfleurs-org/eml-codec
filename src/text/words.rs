use bounded_static::ToStatic;
use crate::print::{Print, Formatter};
use crate::text::ascii;
use crate::text::whitespace::cfws;
use nom::{
    bytes::complete::{tag, take_while1},
    character::is_alphanumeric,
    combinator::{map, opt, recognize},
    multi::many0,
    sequence::{delimited, pair},
    IResult,
};
use std::borrow::Cow;
use std::fmt;

/// ASCII printable characters
pub fn is_vchar(c: u8) -> bool {
    (ascii::EXCLAMATION..=ascii::TILDE).contains(&c)
}

/// A MIME atom.
// Only contains bytes that satisfy `is_mime_atom_text`.
#[derive(Clone, PartialEq, Default, ToStatic)]
pub struct MIMEAtom<'a>(pub Cow<'a, [u8]>);

impl<'a> fmt::Debug for MIMEAtom<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_tuple("MIMEAtom")
            .field(&String::from_utf8_lossy(&self.0))
            .finish()
    }
}
impl<'a> Print for MIMEAtom<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        fmt.write_bytes(&self.0)
    }
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
pub fn mime_atom(input: &[u8]) -> IResult<&[u8], MIMEAtom<'_>> {
    delimited(opt(cfws), mime_atom_plain, opt(cfws))(input)
}

/// `1*token_text`
pub fn mime_atom_plain(input: &[u8]) -> IResult<&[u8], MIMEAtom<'_>> {
    map(take_while1(is_mime_atom_text), |b: &[u8]| MIMEAtom(b.into()))(input)
}

/// An IMF atom.
// Only contains bytes that satisfy `is_atext`.
#[derive(Clone, PartialEq, ToStatic)]
pub struct Atom<'a>(pub Cow<'a, [u8]>);

impl<'a> fmt::Debug for Atom<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_tuple("Atom")
            .field(&String::from_utf8_lossy(&self.0))
            .finish()
    }
}
impl<'a> Print for Atom<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        fmt.write_bytes(&self.0)
    }
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
pub fn atom(input: &[u8]) -> IResult<&[u8], Atom<'_>> {
    map(
        delimited(opt(cfws), take_while1(is_atext), opt(cfws)),
        |b| Atom(b.into()),
    )(input)
}

/// An IMF dot-atom.
// Only contains bytes that satisfy is_atext or are '.'.
#[derive(Clone, PartialEq, ToStatic)]
pub struct DotAtom<'a>(pub Cow<'a, [u8]>);

impl<'a> fmt::Debug for DotAtom<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_tuple("DotAtom")
            .field(&String::from_utf8_lossy(&self.0))
            .finish()
    }
}
impl<'a> Print for DotAtom<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        fmt.write_bytes(&self.0)
    }
}

/// dot-atom-text
///
/// `1*atext *("." 1*atext)`
pub fn dot_atom_text(input: &[u8]) -> IResult<&[u8], DotAtom<'_>> {
    map(
        recognize(pair(
            take_while1(is_atext),
            many0(pair(tag("."), take_while1(is_atext))),
        )),
        |b: &[u8]| DotAtom(b.into()),
    )(input)
}

/// dot-atom
///
/// `[CFWS] dot-atom-text [CFWS]`
#[allow(dead_code)]
pub fn dot_atom(input: &[u8]) -> IResult<&[u8], DotAtom<'_>> {
    delimited(opt(cfws), dot_atom_text, opt(cfws))(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atext() {
        assert!(is_atext(b'='));
        assert!(is_atext(b'5'));
        assert!(is_atext(b'Q'));
        assert!(!is_atext(b' '));
        //assert!(is_atext('É')); // support utf8
    }

    #[test]
    fn test_atom() {
        assert_eq!(
            atom(b"(skip)  imf_codec (hidden) aerogramme"),
            Ok((&b"aerogramme"[..], Atom(b"imf_codec".into())))
        );
    }

    #[test]
    fn test_dot_atom_text() {
        assert_eq!(
            dot_atom_text(b"quentin.dufour.io abcdef"),
            Ok((&b" abcdef"[..], DotAtom(b"quentin.dufour.io".into())))
        );
    }

    #[test]
    fn test_dot_atom() {
        assert_eq!(
            dot_atom(b"   (skip) quentin.dufour.io abcdef"),
            Ok((&b"abcdef"[..], DotAtom(b"quentin.dufour.io".into())))
        );
    }
}
