use nom::{
    IResult,
    bytes::complete::tag,
    sequence::tuple,
    combinator::opt,
};

use crate::text::whitespace::obs_crlf;

#[derive(Debug, PartialEq)]
pub enum Delimiter {
    Next,
    Last
}

pub fn boundary<'a>(boundary: &[u8]) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], Delimiter> + '_ {
    move |input: &[u8]| {
        let (rest, (_, _, _, last, _)) = tuple((opt(obs_crlf), tag(b"--"), tag(boundary), opt(tag(b"--")), opt(obs_crlf)))(input)?;
        match last {
            Some(_) => Ok((rest, Delimiter::Last)),
            None => Ok((rest, Delimiter::Next)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boundary_next() {
        assert_eq!(
            boundary(b"hello")(b"\r\n--hello\r\n"),
            Ok((&b""[..], Delimiter::Next))
        );
    }

    #[test]
    fn test_boundary_last() {
        assert_eq!(
            boundary(b"hello")(b"\r\n--hello--\r\n"),
            Ok((&b""[..], Delimiter::Last))
        );
    }
}
