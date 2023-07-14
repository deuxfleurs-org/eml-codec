use nom::{
    IResult,
    bytes::complete::tag,
    sequence::tuple,
    combinator::opt,
};

use crate::fragments::mime::{Mechanism, Type};
use crate::fragments::model::MessageId;
use crate::fragments::misc_token::Unstructured;
use crate::fragments::whitespace::perm_crlf;

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

pub fn boundary(boundary: &[u8]) -> impl Fn(&[u8]) -> IResult<&[u8], Delimiter> {
    |input: &[u8]| {
        let (_, _, _, last, _) = tuple((perm_crlf, tag(b"--"), tag(boundary), opt(tag(b"--")), perm_crlf))(input)?;
        match last {
            Some(_) => Delimiter::Last,
            None => Delimiter::Next,
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
