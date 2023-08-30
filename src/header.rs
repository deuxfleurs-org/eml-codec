use std::fmt;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_while1},
    character::complete::space0,
    combinator::{into, recognize},
    multi::many0,
    sequence::{pair, terminated, tuple},
    IResult,
};

use crate::text::whitespace::{foldable_line, obs_crlf};
use crate::text::misc_token::unstructured;

#[derive(PartialEq, Clone)]
pub struct Kv2<'a>(pub &'a [u8], pub &'a [u8]);
impl<'a> From<(&'a [u8], &'a [u8])> for Kv2<'a> {
    fn from(pair: (&'a [u8], &'a [u8])) -> Self {
        Self(pair.0, pair.1)
    }
}
impl<'a> fmt::Debug for Kv2<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_tuple("header::Kv2")
            .field(&String::from_utf8_lossy(self.0))
            .field(&String::from_utf8_lossy(self.1))
            .finish()
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum Field<'a> {
    Good(Kv2<'a>),
    Bad(&'a [u8]),
}
impl<'a> From<Kv2<'a>> for Field<'a> {
    fn from(kv: Kv2<'a>) -> Self {
        Self::Good(kv)
    }
}
impl<'a> From<&'a [u8]> for Field<'a> {
    fn from(bad: &'a [u8]) -> Self {
        Self::Bad(bad)
    }
}

/// Parse headers as key/values
pub fn header_kv(input: &[u8]) -> IResult<&[u8], Vec<Field>> {
    terminated(
        many0(
            alt((
                into(correct_field),
                into(foldable_line),
            ))
        ),
        obs_crlf
    )(input)
}

pub fn field_any(input: &[u8]) -> IResult<&[u8], &[u8]> {
    terminated(
        take_while1(|c| (0x21..=0x7E).contains(&c) && c != 0x3A),
        tuple((space0, tag(b":"), space0)),
    )(input)
}

/// Optional field
///
/// ```abnf
/// field      =   field-name ":" unstructured CRLF
/// field-name =   1*ftext
/// ftext      =   %d33-57 /          ; Printable US-ASCII
///                %d59-126           ;  characters not including
///                                   ;  ":".
/// ```
pub fn correct_field(input: &[u8]) -> IResult<&[u8], Kv2> {
    terminated(
        into(pair(
            field_any,
            recognize(unstructured),
        )),
        obs_crlf,
    )(input)
}
