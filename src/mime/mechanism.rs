use crate::text::whitespace::cfws;
use crate::text::words::mime_atom as token;
use nom::{
    branch::alt,
    bytes::complete::tag_no_case,
    combinator::{map, opt, value},
    sequence::delimited,
    IResult,
};

#[derive(Debug, Clone, PartialEq, Default)]
pub enum Mechanism<'a> {
    #[default]
    _7Bit,
    _8Bit,
    Binary,
    QuotedPrintable,
    Base64,
    Other(&'a [u8]),
}
impl<'a> ToString for Mechanism<'a> {
    fn to_string(&self) -> String {
        use Mechanism::*;
        let buf: &[u8] = match self {
            _7Bit => b"7bit",
            _8Bit => b"8bit",
            Binary => b"binary",
            QuotedPrintable => b"quoted-printable",
            Base64 => b"base64",
            Other(x) => x,
        };
        String::from_utf8_lossy(buf).to_string()
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
        map(token, Other),
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
