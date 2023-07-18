use nom::{
    bytes::complete::is_not,
    character::complete::space1,
    combinator::{all_consuming, recognize},
    multi::{many0, many1},
    sequence::{pair, tuple},
    IResult,
};

use crate::error::IMFError;
use crate::fragments::fields;
use crate::multipass::field_lazy;
use crate::multipass::guess_charset;

#[derive(Debug, PartialEq)]
pub struct Parsed<'a> {
    pub fields: Vec<&'a str>,
    pub body: &'a [u8],
}

pub fn new<'a>(gcha: &'a guess_charset::Parsed<'a>) -> Result<Parsed<'a>, IMFError<'a>> {
    fields(&gcha.header)
        .map_err(|e| IMFError::ExtractFields(e))
        .map(|(_, fields)| Parsed {
            fields,
            body: gcha.body,
        })
}

impl<'a> Parsed<'a> {
    pub fn names(&'a self) -> field_lazy::Parsed<'a> {
        field_lazy::new(self)
    }
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
