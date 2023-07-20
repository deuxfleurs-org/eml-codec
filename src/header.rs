use nom::{
    IResult,
    bytes::complete::{tag_no_case, tag, take_while1},
    character::complete::space0,
    sequence::{pair, terminated, tuple},
};
use crate::text::misc_token::{Unstructured, unstructured};

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
        self.0.into_iter().map(|v| match v {
            CompField::Known(f) => Some(f),
            _ => None,
        }).flatten().collect()
    }
}

pub fn field_name<'a>(name: &'static [u8]) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], &'a [u8]> {
    move |input| {
        terminated(
            tag_no_case(name),
            tuple((space0, tag(b":"), space0)),
        )(input)
    }
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
    pair(
        terminated(
            take_while1(|c| c >= 0x21 && c <= 0x7E && c != 0x3A),
            tuple((space0, tag(b":"), space0)),
        ),
        unstructured,
    )(input)
}


