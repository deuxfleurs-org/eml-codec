use nom::{
    branch::alt,
    bytes::complete::{tag, take_while1},
    combinator::{recognize, verify},
    multi::many0,
    IResult,
};
use crate::text::quoted::quoted_string_plain;
use crate::text::encoding::{Context, encoded_word_token};

// Helper combinators that help skipping over part of the input. This is useful
// when recovering from ill-formatted input.

pub fn take_quoted_or_until<F>(pred: F) -> impl FnMut(&[u8]) -> IResult<&[u8], &[u8]>
    where F: Fn(u8) -> bool
{
    move |input: &[u8]| {
        recognize(
            many0(alt((
                take_while1(|c| c != b'"' && !pred(c)),
                recognize(quoted_string_plain),
                tag("\""), // fallback if `quoted_string` failed
            )))
        )(input)
    }
}

pub fn take_quoted_or_until1<F>(pred: F) -> impl FnMut(&[u8]) -> IResult<&[u8], &[u8]>
    where F: Fn(u8) -> bool
{
    move |input: &[u8]| {
        verify(take_quoted_or_until(&pred), |i: &[u8]| !i.is_empty())(input)
    }
}

pub fn take_quoted_encoded_or_until<F>(pred: F) -> impl FnMut(&[u8]) -> IResult<&[u8], &[u8]>
    where F: Fn(u8) -> bool
{
    move |input: &[u8]| {
        let res =
        recognize(
            many0(
                alt((
                    take_while1(|c| c != b'"' && c != b'=' && !pred(c)),
                    recognize(quoted_string_plain),
                    tag("\""), // fallback if `quoted_string` failed
                    // hardcode the context for now...
                    recognize(encoded_word_token(Context::Phrase)),
                    tag("="), // fallback if `encoded_word_token` failed
                )),
            )
        )(input)?;
        Ok(res)
    }
}

pub fn take_quoted_encoded_or_until1<F>(pred: F) -> impl FnMut(&[u8]) -> IResult<&[u8], &[u8]>
    where F: Fn(u8) -> bool
{
    move |input: &[u8]| {
        verify(take_quoted_encoded_or_until(&pred), |i: &[u8]| !i.is_empty())(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn take_quoted() {
        assert_eq!(
            take_quoted_or_until(|c| c == b'!')(b"abc\"de!fg\"!hi"),
            Ok((&b"!hi"[..], &b"abc\"de!fg\""[..]))
        );

        // consume the input to the end if the condition is not met
        assert_eq!(
            take_quoted_or_until(|c| c == b'!')(b"abc\"de!fg\"hi"),
            Ok((&b""[..], &b"abc\"de!fg\"hi"[..]))
        );

        // start quote without end quote: ignore it and continue
        assert_eq!(
            take_quoted_or_until(|c| c == b'!')(b"abc\"de!fg!hi"),
            Ok((&b"!fg!hi"[..], &b"abc\"de"[..]))
        );
    }

    #[test]
    fn take_quoted_encoded() {
        assert_eq!(
            take_quoted_encoded_or_until(|c| c == b'!')(b"abc\"de!fg\"=?utf-8?q?a!!bc?= !hi"),
            Ok((&b"!hi"[..], &b"abc\"de!fg\"=?utf-8?q?a!!bc?= "[..]))
        );

        // broken quote or encoded word: ignore it and continue
        assert_eq!(
            take_quoted_encoded_or_until(|c| c == b'!')(b"abc\"de=?utf-8?q?uu!fg!hi"),
            Ok((&b"!fg!hi"[..], &b"abc\"de=?utf-8?q?uu"[..]))
        );
    }
}
