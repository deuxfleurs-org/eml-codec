use std::borrow::Cow;
use nom::{
    IResult,
    character::complete::space1,
    bytes::complete::is_not,
    combinator::{all_consuming, recognize},
    multi::{fold_many0, many0, many1},
    sequence::{pair, tuple},
};

use crate::multipass::guess_charset::GuessCharset;
use crate::error::IMFError;
use crate::fragments::whitespace;

#[derive(Debug, PartialEq)]
pub struct ExtractFields<'a> {
    pub fields: Vec<&'a str>,
    pub body: &'a [u8],
}

impl<'a> TryFrom<&'a GuessCharset<'a>> for ExtractFields<'a> {
    type Error = IMFError<'a>;

    fn try_from(gcha: &'a GuessCharset<'a>) -> Result<Self, Self::Error> {
        all_consuming(many0(foldable_line))(&gcha.header)
            .map_err(|e| IMFError::ExtractFields(e))
            .map(|(_, fields)| ExtractFields { fields, body: gcha.body })
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
            ExtractFields::try_from(&GuessCharset {
                header: "From: hello@world.com,\r\n\talice@wonderlands.com\r\nDate: 12 Mar 1997 07:33:25 Z\r\n".into(),
                encoding: encoding_rs::UTF_8,
                malformed: false,
                body: b"Hello world!",
            }),
            Ok(ExtractFields {
                fields: vec![
                    "From: hello@world.com,\r\n\talice@wonderlands.com\r\n",
                    "Date: 12 Mar 1997 07:33:25 Z\r\n",
                ],
                body: b"Hello world!",
            })
        );
    }
}
