#[cfg(feature = "arbitrary")]
use arbitrary::Arbitrary;
use bounded_static::ToStatic;
#[cfg(feature = "arbitrary")]
use crate::{
    arbitrary_utils::{
        arbitrary_vec_nonempty_where,
        arbitrary_string_nonempty_where,
    },
    fuzz_eq::FuzzEq,
};
use crate::print::{Print, Formatter};
use crate::text::ascii;
use crate::text::whitespace::cfws;
use crate::utils::{is_nonascii_or, take_utf8_while1, ContainsUtf8};
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

/// Printable characters
///
/// following RFC6532, this includes non-ascii UTF8 text
pub fn is_vchar(c: char) -> bool {
    is_nonascii_or(|c| (ascii::EXCLAMATION..=ascii::TILDE).contains(&c))(c)
}

/// A MIME atom.
// Contains a non-zero amount of bytes that satisfy `is_mime_atom_text`.
#[derive(Clone, ContainsUtf8, PartialEq, Default, ToStatic)]
#[contains_utf8(false)]
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
#[cfg(feature = "arbitrary")]
impl<'a, 'b> Arbitrary<'a> for MIMEAtom<'b> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<MIMEAtom<'b>> {
        let bytes = arbitrary_vec_nonempty_where(u, |c| is_mime_atom_text(*c), b'X')?;
        Ok(MIMEAtom(Cow::Owned(bytes)))
    }
}
#[cfg(feature = "arbitrary")]
impl<'a> FuzzEq for MIMEAtom<'a> {
    fn fuzz_eq(&self, other: &Self) -> bool {
        self == other
    }
}
impl<'a> MIMEAtom<'a> {
    pub fn chars<'b>(&'b self) -> MIMEAtomChars<'a, 'b> {
        MIMEAtomChars { a: &self, idx: 0 }
    }
}
#[derive(Clone)]
pub struct MIMEAtomChars<'a, 'b> {
    a: &'b MIMEAtom<'a>,
    idx: usize,
}
impl<'a, 'b> Iterator for MIMEAtomChars<'a, 'b> {
    type Item = char;
    fn next(&mut self) -> Option<Self::Item> {
        if self.idx < self.a.0.len() {
            let c: u8 = self.a.0[self.idx].into();
            self.idx += 1;
            Some(c.into())
        } else {
            None
        }
    }
}

/// MIME Token allowed characters
///
/// forbidden: ()<>@,;:\"/[]?=
pub fn is_mime_atom_text(c: u8) -> bool {
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
// Contains a non-zero amount of bytes that satisfy `is_atext`.
#[derive(Clone, Debug, PartialEq, ToStatic)]
pub struct Atom<'a>(pub Cow<'a, str>);

impl<'a> Print for Atom<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        fmt.write_bytes(&self.0.as_bytes())
    }
}
#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for Atom<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Atom<'a>> {
        let bytes = arbitrary_string_nonempty_where(u, is_atext, 'X')?;
        Ok(Atom(Cow::Owned(bytes)))
    }
}
#[cfg(feature = "arbitrary")]
impl<'a> FuzzEq for Atom<'a> {
    fn fuzz_eq(&self, other: &Self) -> bool {
        self == other
    }
}

/// Atom allowed characters
///
/// authorized: !#$%&'*+-/=?^_`{|}~
///
/// following RFC6532, atext also allows non-ascii UTF8 characters
pub fn is_atext(c: char) -> bool {
    is_nonascii_or(|c| {
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
    })(c)
}

/// Atom
///
/// `[CFWS] 1*atext [CFWS]`
pub fn atom(input: &[u8]) -> IResult<&[u8], Atom<'_>> {
    map(
        delimited(opt(cfws), take_utf8_while1(is_atext), opt(cfws)),
        |b| Atom(b.into()),
    )(input)
}

/// An IMF dot-atom.
// Only contains bytes that satisfy is_atext or are '.'.
#[derive(Clone, ContainsUtf8, Debug, PartialEq, ToStatic)]
pub struct DotAtom<'a>(pub Cow<'a, str>);

impl<'a> Print for DotAtom<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        fmt.write_bytes(&self.0.as_bytes())
    }
}
#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for DotAtom<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<DotAtom<'a>> {
        let mut s = arbitrary_string_nonempty_where(u, is_atext, 'X')?;
        for _ in 0..u.int_in_range(0..=3)? {
            s.push('.');
            s.push_str(&arbitrary_string_nonempty_where(u, is_atext, 'X')?);
        }
        Ok(DotAtom(Cow::Owned(s)))
    }
}
#[cfg(feature = "arbitrary")]
impl<'a> FuzzEq for DotAtom<'a> {
    fn fuzz_eq(&self, other: &Self) -> bool {
        self == other
    }
}

/// dot-atom-text
///
/// `1*atext *("." 1*atext)`
pub fn dot_atom_text(input: &[u8]) -> IResult<&[u8], DotAtom<'_>> {
    map(
        recognize(pair(
            take_utf8_while1(is_atext),
            many0(pair(tag("."), take_utf8_while1(is_atext))),
        )),
        |b: &[u8]| {
            let s = unsafe { str::from_utf8_unchecked(b) };
            DotAtom(s.into())
        },
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
        assert!(is_atext('='));
        assert!(is_atext('5'));
        assert!(is_atext('Q'));
        assert!(!is_atext(' '));
        assert!(is_atext('É')); // non-ascii UTF8 is allowed (RFC6532)
    }

    #[test]
    fn test_atom() {
        assert_eq!(
            atom(b"(skip)  imf_codec (hidden) aerogramme"),
            Ok((&b"aerogramme"[..], Atom("imf_codec".into())))
        );
    }

    #[test]
    fn test_dot_atom_text() {
        assert_eq!(
            dot_atom_text(b"quentin.dufour.io abcdef"),
            Ok((&b" abcdef"[..], DotAtom("quentin.dufour.io".into())))
        );
    }

    #[test]
    fn test_dot_atom() {
        assert_eq!(
            dot_atom(b"   (skip) quentin.dufour.io abcdef"),
            Ok((&b"abcdef"[..], DotAtom("quentin.dufour.io".into())))
        );
    }
}
