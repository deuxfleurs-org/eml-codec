use nom::{
    IResult,
    bytes::complete::tag,
    sequence::tuple,
    combinator::opt,
};

use crate::fragments::mime::{Mechanism, Type};
use crate::fragments::model::MessageId;
use crate::fragments::misc_token::Unstructured;
use crate::fragments::whitespace::obs_crlf;

#[derive(Debug, PartialEq, Default)]
pub struct PartHeader<'a> {
    pub content_type: Option<&'a Type<'a>>,
    pub content_transfer_encoding: Option<&'a Mechanism<'a>>,
    pub content_id: Option<&'a MessageId<'a>>,
    pub content_description: Option<&'a Unstructured>,
}

#[derive(Debug, PartialEq)]
pub enum PartNode<'a> {
    Discrete(PartHeader<'a>, &'a [u8]),
    Composite(PartHeader<'a>, Vec<PartNode<'a>>),
}

pub enum Delimiter {
    Next,
    Last
}

pub fn boundary<'a>(boundary: &'a [u8]) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], Delimiter> {
    move |input: &[u8]| {
        let (rest, (_, _, _, last, _)) = tuple((obs_crlf, tag(b"--"), tag(boundary), opt(tag(b"--")), obs_crlf))(input)?;
        match last {
            Some(_) => Ok((rest, Delimiter::Last)),
            None => Ok((rest, Delimiter::Next)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boundary() {
    }
}
