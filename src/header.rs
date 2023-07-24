use crate::text::misc_token::{unstructured, Unstructured};
use crate::text::whitespace::{foldable_line, obs_crlf};
use nom::{
    branch::alt,
    bytes::complete::{tag, tag_no_case, take_while1},
    character::complete::space0,
    combinator::map,
    multi::many0,
    sequence::{pair, terminated, tuple},
    IResult,
};

#[derive(Debug, PartialEq)]
pub enum CompField<'a, T> {
    Known(T),
    Unknown(&'a [u8], Unstructured<'a>),
    Bad(&'a [u8]),
}

#[derive(Debug, PartialEq)]
pub struct CompFieldList<'a, T>(pub Vec<CompField<'a, T>>);
impl<'a, T> CompFieldList<'a, T> {
    pub fn known(self) -> Vec<T> {
        self.0
            .into_iter()
            .filter_map(|v| match v {
                CompField::Known(f) => Some(f),
                _ => None,
            })
            .collect()
    }
}

pub fn header<'a, T>(
    fx: impl Fn(&'a [u8]) -> IResult<&'a [u8], T> + Copy,
) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], CompFieldList<T>> {
    move |input| {
        map(
            terminated(
                many0(alt((
                    map(fx, CompField::Known),
                    map(opt_field, |(k, v)| CompField::Unknown(k, v)),
                    map(foldable_line, CompField::Bad),
                ))),
                obs_crlf,
            ),
            CompFieldList,
        )(input)
    }
}

pub fn field_name<'a>(name: &'static [u8]) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], &'a [u8]> {
    move |input| terminated(tag_no_case(name), tuple((space0, tag(b":"), space0)))(input)
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
pub fn opt_field(input: &[u8]) -> IResult<&[u8], (&[u8], Unstructured)> {
    terminated(
        pair(
            terminated(
                take_while1(|c| (0x21..=0x7E).contains(&c) && c != 0x3A),
                tuple((space0, tag(b":"), space0)),
            ),
            unstructured,
        ),
        obs_crlf,
    )(input)
}
