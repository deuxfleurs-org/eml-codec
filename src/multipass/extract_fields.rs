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
    pub raw_header: Cow<'a, str>,
    pub fields: Vec<&'a str>,
    pub body: &'a [u8],
}

impl<'a> TryFrom<GuessCharset<'a>> for ExtractFields<'a> {
    type Error = IMFError<'a>;

    fn try_from(gcha: GuessCharset<'a>) -> Result<Self, Self::Error> {
        let mut ef = ExtractFields { fields: vec![], raw_header: gcha.header, body: gcha.body };
        let (_, fields) = all_consuming(many0(foldable_line))(ef.raw_header).map_err(|e| IMFError::ExtractFields(e))?;
        panic!();
        //ef.fields = fields;
        //Ok(ef)
    }
}

/// ```abnf
/// fold_line = !crlf *(1*(crlf WS) !crlf) crlf
/// ```
fn foldable_line<'a>(input: Cow<'a, str>) -> IResult<Cow<'a, str>, Cow<'a, str>> {
    recognize(tuple((
        is_not("\r\n"), 
        many0(pair(
                many1(pair(whitespace::perm_crlf, space1)), 
                is_not("\r\n"))), 
        whitespace::perm_crlf
    )))(input)
}
