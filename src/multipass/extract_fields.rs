use nom::{
    IResult,
    character::complete::space1,
    bytes::complete::is_not,
    combinator::{all_consuming, recognize},
    multi::{many0, many1},
    sequence::{pair, tuple},
};

use crate::error::IMFError;
use crate::fragments::whitespace;
use crate::multipass::guess_charset;
use crate::multipass::field_lazy;

#[derive(Debug, PartialEq)]
pub struct Parsed<'a> {
    pub fields: Vec<&'a str>,
    pub body: &'a [u8],
}

pub fn new<'a>(gcha: &'a guess_charset::Parsed<'a>) -> Result<Parsed<'a>, IMFError<'a>> {
    all_consuming(many0(foldable_line))(&gcha.header)
        .map_err(|e| IMFError::ExtractFields(e))
        .map(|(_, fields)| Parsed { fields, body: gcha.body })
}

impl<'a> Parsed<'a> {
    pub fn names(&'a self) -> field_lazy::Parsed<'a> {
        field_lazy::new(self)
    }
}

/// ```abnf
/// fold_line = any *(1*(crlf WS) any) crlf
/// ```
fn foldable_line(input: &str) -> IResult<&str, &str> {
    recognize(tuple((
        is_not("\r\n"), 
        many0(pair(
                many1(pair(whitespace::perm_crlf, space1)), 
                is_not("\r\n"))), 
        whitespace::perm_crlf
    )))(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract() {
        assert_eq!(
            new(&guess_charset::Parsed {
                header: "From: hello@world.com,\r\n\talice@wonderlands.com\r\nDate: 12 Mar 1997 07:33:25 Z\r\n".into(),
                encoding: encoding_rs::UTF_8,
                malformed: false,
                body: b"Hello world!",
            }),
            Ok(Parsed {
                fields: vec![
                    "From: hello@world.com,\r\n\talice@wonderlands.com\r\n",
                    "Date: 12 Mar 1997 07:33:25 Z\r\n",
                ],
                body: b"Hello world!",
            })
        );
    }
}
