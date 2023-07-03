use encoding_rs::Encoding;
use nom::{
    bytes::complete::tag, character::complete as character, combinator::opt, sequence::tuple,
    IResult,
};

use crate::error::IMFError;
use crate::fragments::lazy;
use crate::fragments::whitespace::cfws;

#[derive(Debug, PartialEq)]
pub struct Version {
    major: u32,
    minor: u32,
}

#[derive(Debug, PartialEq)]
pub enum Type<'a> {
    Multipart(MultipartSubtype<'a>),
    Message(MessageSubtype<'a>),
    Other(&'a str, &'a str, Vec<Parameter<'a>>),
}

#[derive(Debug, PartialEq)]
pub enum MultipartSubtype<'a> {
    Alternative(Parameter<'a>),
    Mixed(Parameter<'a>),
    Digest(Parameter<'a>),
    Parallel(Parameter<'a>),
    Other(&'a str, Parameter<'a>),
}

#[derive(Debug, PartialEq)]
pub enum MessageSubtype<'a> {
    RFC822(Vec<Parameter<'a>>),
    Partial(Vec<Parameter<'a>>),
    External(Vec<Parameter<'a>>),
    Other(&'a str, Vec<Parameter<'a>>),
}

#[derive(Debug, PartialEq)]
pub enum Parameter<'a> {
    Charset(&'static Encoding),
    Boundary(&'a str),
    Other(&'a str, &'a str),
}

#[derive(Debug, PartialEq)]
pub enum Mechanism<'a> {
    _7Bit,
    _8Bit,
    Binary,
    QuotedPrintable,
    Base64,
    Other(&'a str),
}

impl<'a> TryFrom<&'a lazy::Version<'a>> for Version {
    type Error = IMFError<'a>;

    fn try_from(vs: &'a lazy::Version<'a>) -> Result<Self, Self::Error> {
        version(vs.0)
            .map(|(_, v)| v)
            .map_err(|e| IMFError::Version(e))
    }
}

impl<'a> TryFrom<&'a lazy::Type<'a>> for Type<'a> {
    type Error = IMFError<'a>;

    fn try_from(tp: &'a lazy::Type<'a>) -> Result<Self, Self::Error> {
        Ok(Type::Other("", "", vec![]))
    }
}

impl<'a> TryFrom<&'a lazy::Mechanism<'a>> for Mechanism<'a> {
    type Error = IMFError<'a>;

    fn try_from(mc: &'a lazy::Mechanism<'a>) -> Result<Self, Self::Error> {
        Ok(Mechanism::Other(""))
    }
}

pub fn version(input: &str) -> IResult<&str, Version> {
    let (rest, (_, major, _, _, _, minor, _)) = tuple((
        opt(cfws),
        character::u32,
        opt(cfws),
        tag("."),
        opt(cfws),
        character::u32,
        opt(cfws),
    ))(input)?;
    Ok((rest, Version { major, minor }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert_eq!(version("1.0"), Ok(("", Version { major: 1, minor: 0 })),);

        assert_eq!(
            version(" 1.0 (produced by MetaSend Vx.x)"),
            Ok(("", Version { major: 1, minor: 0 })),
        );

        assert_eq!(
            version("(produced by MetaSend Vx.x) 1.0"),
            Ok(("", Version { major: 1, minor: 0 })),
        );

        assert_eq!(
            version("1.(produced by MetaSend Vx.x)0"),
            Ok(("", Version { major: 1, minor: 0 })),
        );
    }
}
