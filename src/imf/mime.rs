#[cfg(feature = "arbitrary")]
use arbitrary::Arbitrary;
use bounded_static::ToStatic;
use nom::{
    bytes::complete::tag,
    character::complete::digit1,
    combinator::{map, opt},
    sequence::tuple,
    IResult,
};

#[cfg(feature = "arbitrary")]
use crate::fuzz_eq::FuzzEq;
use crate::i18n::ContainsUtf8;
use crate::print::{Formatter, Print, ToStringFromPrint};
use crate::text::whitespace::cfws;
use eml_codec_derives::instrument_input;

#[derive(Clone, ContainsUtf8, Debug, PartialEq, ToStatic, ToStringFromPrint)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
#[contains_utf8(false)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
}

impl Default for Version {
    fn default() -> Self {
        Self { major: 1, minor: 0 }
    }
}
#[cfg(feature = "arbitrary")]
impl FuzzEq for Version {
    fn fuzz_eq(&self, other: &Self) -> bool {
        self == other
    }
}

#[instrument_input("tracing")]
pub fn version(input: &[u8]) -> IResult<&[u8], Version> {
    let (rest, (_, major, _, _, _, minor, _)) = tuple((
        opt(cfws),
        map(digit1, ascii_to_u64),
        opt(cfws),
        tag(b"."),
        opt(cfws),
        map(digit1, ascii_to_u64),
        opt(cfws),
    ))(input)?;
    Ok((rest, Version { major, minor }))
}

fn ascii_to_u64(c: &[u8]) -> u64 {
    str::from_utf8(c).unwrap().parse().unwrap()
}

impl Print for Version {
    fn print(&self, fmt: &mut impl Formatter) {
        fmt.write_bytes(self.major.to_string().as_bytes());
        fmt.write_bytes(b".");
        fmt.write_bytes(self.minor.to_string().as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert_eq!(
            version(b"1.0"),
            Ok((&b""[..], Version { major: 1, minor: 0 })),
        );

        assert_eq!(
            version(b" 1.0 (produced by MetaSend Vx.x)"),
            Ok((&b""[..], Version { major: 1, minor: 0 })),
        );

        assert_eq!(
            version(b"(produced by MetaSend Vx.x) 1.0"),
            Ok((&b""[..], Version { major: 1, minor: 0 })),
        );

        assert_eq!(
            version(b"1.(produced by MetaSend Vx.x)0"),
            Ok((&b""[..], Version { major: 1, minor: 0 })),
        );
    }
}
