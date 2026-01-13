use bounded_static::ToStatic;
use crate::print::{Print, Formatter};
use crate::text::whitespace::cfws;
use crate::text::words::mime_atom as token;
use nom::{
    branch::alt,
    bytes::complete::tag_no_case,
    combinator::{map, opt, value},
    sequence::delimited,
    IResult,
};
use std::borrow::Cow;

#[derive(Debug, Clone, PartialEq, Default, ToStatic)]
pub enum Mechanism<'a> {
    #[default]
    _7Bit,
    _8Bit,
    Binary,
    QuotedPrintable,
    Base64,
    Other(Cow<'a, [u8]>),
}
impl<'a> Mechanism<'a> {
    pub fn as_bytes(&self) -> &[u8] {
        use Mechanism::*;
        match self {
            _7Bit => b"7bit",
            _8Bit => b"8bit",
            Binary => b"binary",
            QuotedPrintable => b"quoted-printable",
            Base64 => b"base64",
            Other(x) => &x,
        }
    }
}

impl<'a> ToString for Mechanism<'a> {
    fn to_string(&self) -> String {
        String::from_utf8_lossy(self.as_bytes()).to_string()
    }
}
impl<'a> Print for Mechanism<'a> {
    fn print(&self, fmt: &mut impl Formatter) -> std::io::Result<()> {
        fmt.write_bytes(self.as_bytes())
    }
}
impl<'a> Mechanism<'a> {
    // RFC2046: for entities of type "multipart" or "message/rfc822",
    // no encoding other than 7bit, 8bit and binary is permitted.
    // This converts a `Mechanism` to ensure it belongs to
    // one of these three encodings, defaulting to 7bit in case
    // of an invalid value.
    pub fn to_part_encoding(&self) -> Mechanism<'static> {
        match self {
            Mechanism::_8Bit => Mechanism::_8Bit,
            Mechanism::Binary => Mechanism::Binary,
            _ => Mechanism::_7Bit,
        }
    }
}

pub fn mechanism(input: &[u8]) -> IResult<&[u8], Mechanism<'_>> {
    use Mechanism::*;

    alt((
        delimited(
            opt(cfws),
            alt((
                value(_7Bit, tag_no_case("7bit")),
                value(_8Bit, tag_no_case("8bit")),
                value(Binary, tag_no_case("binary")),
                value(QuotedPrintable, tag_no_case("quoted-printable")),
                value(Base64, tag_no_case("base64")),
            )),
            opt(cfws),
        ),
        map(token, |b| Other(Cow::Borrowed(b))),
    ))(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_mechanism() {
        assert_eq!(mechanism(b"7bit"), Ok((&b""[..], Mechanism::_7Bit)),);

        assert_eq!(
            mechanism(b"(youhou) 8bit"),
            Ok((&b""[..], Mechanism::_8Bit)),
        );

        assert_eq!(
            mechanism(b"(blip) bInArY (blip blip)"),
            Ok((&b""[..], Mechanism::Binary)),
        );

        assert_eq!(mechanism(b" base64 "), Ok((&b""[..], Mechanism::Base64)),);

        assert_eq!(
            mechanism(b" Quoted-Printable "),
            Ok((&b""[..], Mechanism::QuotedPrintable)),
        );
    }
}
