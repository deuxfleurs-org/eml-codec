#[cfg(feature = "arbitrary")]
use arbitrary::Arbitrary;
use bounded_static::ToStatic;
#[cfg(feature = "tracing")]
use tracing::warn;
#[cfg(feature = "arbitrary")]
use crate::fuzz_eq::FuzzEq;
#[cfg(feature = "tracing")]
use crate::utils::bytes_to_display_string;
use crate::i18n::ContainsUtf8;
use crate::print::{Print, Formatter, ToStringFromPrint};
use crate::text::whitespace::cfws;
use crate::text::words::{mime_atom, MIMEAtom};
use nom::{
    branch::alt,
    bytes::complete::tag_no_case,
    combinator::{consumed, map, opt, value},
    sequence::delimited,
    IResult,
};

#[derive(Debug, Clone, PartialEq, Default, ToStatic, ToStringFromPrint)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary, FuzzEq))]
pub enum Mechanism<'a> {
    #[default]
    _7Bit,
    _8Bit,
    Binary,
    QuotedPrintable,
    Base64,
    Other(MIMEAtom<'a>),
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
            Other(x) => &x.0,
        }
    }
}

impl<'a> Print for Mechanism<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        fmt.write_bytes(self.as_bytes())
    }
}
impl<'a> ContainsUtf8 for Mechanism<'a> {
    fn contains_utf8(&self) -> bool {
        false
    }
}
impl<'a> Mechanism<'a> {
    // RFC2046: for entities of type "multipart" or "message/rfc822",
    // no encoding other than 7bit, 8bit and binary is permitted.
    // This converts a `Mechanism` to ensure it belongs to
    // one of these three encodings, returning the default mechanism
    // in case of an invalid value.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace")
    )]
    pub fn to_part_encoding(&self) -> Mechanism<'static> {
        use bounded_static::ToBoundedStatic;
        match self {
            Mechanism::_7Bit | Mechanism::_8Bit | Mechanism::Binary =>
                self.to_static(),
            _ => {
                #[cfg(feature = "tracing")]
                warn!(mechanism = ?self, "invalid mechanism");
                Mechanism::default()
            }
        }
    }
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = bytes_to_display_string(input)))
)]
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
        map(consumed(mime_atom), |(_i, tok)| {
            #[cfg(feature = "tracing")]
            warn!(input = bytes_to_display_string(_i), "unknown mechanism");
            Other(tok)
        }),
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
