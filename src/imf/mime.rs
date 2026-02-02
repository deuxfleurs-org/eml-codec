use bounded_static::ToStatic;
use nom::{
    character::complete::digit1,
    bytes::complete::tag,
    combinator::{map, opt},
    sequence::tuple,
    IResult,
};

use crate::print::{Print, Formatter};
use crate::text::whitespace::cfws;

#[derive(Debug, PartialEq, ToStatic)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
}

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
    fn print(&self, fmt: &mut impl Formatter) -> std::io::Result<()> {
        fmt.write_bytes(self.major.to_string().as_bytes())?;
        fmt.write_bytes(b".")?;
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
