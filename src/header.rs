
use crate::text::misc_token::{unstructured, Unstructured};
use crate::text::whitespace::{foldable_line, obs_crlf};
use nom::{
    branch::alt,
    bytes::complete::{tag, tag_no_case, take_while1},
    character::complete::space0,
    combinator::{into, map},
    multi::{fold_many0, many0},
    sequence::{pair, terminated, tuple},
    IResult,
};

#[derive(Debug, PartialEq)]
pub enum CompField<'a, T> {
    Known(T),
    Unknown(Kv<'a>),
    Bad(&'a [u8]),
}

#[derive(Debug, PartialEq, Clone)]
pub struct Kv<'a>(pub &'a [u8], pub Unstructured<'a>);
impl<'a> From<(&'a [u8], Unstructured<'a>)> for Kv<'a> {
    fn from(pair: (&'a [u8], Unstructured<'a>)) -> Self {
        Self(pair.0, pair.1)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum Field<'a> {
    Good(Kv<'a>),
    Bad(&'a [u8]),
}
impl<'a> From<Kv<'a>> for Field<'a> {
    fn from(kv: Kv<'a>) -> Self {
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
                into(opt_field),
                into(foldable_line),
            ))
        ),
        obs_crlf
    )(input)
}


pub fn header<'a, T>(
    fx: impl Fn(&'a [u8]) -> IResult<&'a [u8], T> + Copy,
) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], (Vec::<T>, Vec::<Kv>, Vec<&'a [u8]>)> {
    move |input| {
            terminated(
                fold_many0(
                    alt((
                        map(fx, CompField::Known),
                        map(opt_field, CompField::Unknown),
                        map(foldable_line, CompField::Bad),
                    )),
                    || (Vec::<T>::new(), Vec::<Kv>::new(), Vec::<&'a [u8]>::new()),
                    |(mut known, mut unknown, mut bad), item| {
                        match item {
                            CompField::Known(v) => known.push(v),
                            CompField::Unknown(v) => unknown.push(v),
                            CompField::Bad(v) => bad.push(v),
                        };
                        (known, unknown, bad)
                    }
                ),
                obs_crlf,
            )(input)
    }
}

pub fn field_name<'a>(name: &'static [u8]) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], &'a [u8]> {
    move |input| terminated(tag_no_case(name), tuple((space0, tag(b":"), space0)))(input)
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
pub fn opt_field(input: &[u8]) -> IResult<&[u8], Kv> {
    terminated(
        into(pair(
            field_any,
            unstructured,
        )),
        obs_crlf,
    )(input)
}
