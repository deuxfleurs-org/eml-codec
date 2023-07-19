use nom::{
    IResult,
    sequence::tuple,
    bytes::complete::{tag, take},
    combinator::{map, opt, verify},
};

use crate::text::ascii;
use crate::text::whitespace::cfws;

#[derive(Debug, PartialEq)]
pub struct Version {
    pub major: u8,
    pub minor: u8,
}

pub fn version(input: &[u8]) -> IResult<&[u8], Version> {
    let (rest, (_, major, _, _, _, minor, _)) = tuple((
        opt(cfws),
        map(verify(take(1usize), is_digit), ascii_to_u8),
        opt(cfws),
        tag(b"."),
        opt(cfws),
        map(verify(take(1usize), is_digit), ascii_to_u8),
        opt(cfws),
    ))(input)?;
    Ok((rest, Version { major, minor }))
}

fn is_digit(c: &[u8]) -> bool {
    c[0] >= ascii::N0 && c[0] <= ascii::N9
}

fn ascii_to_u8(c: &[u8]) -> u8 {
    c[0] - ascii::N0
}
