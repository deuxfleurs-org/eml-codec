#[cfg(feature = "arbitrary")]
use arbitrary::Arbitrary;
#[cfg(feature = "tracing")]
use tracing::warn;
use bounded_static::ToStatic;
use nom::{
    branch::alt,
    bytes::complete::tag,
    combinator::{consumed, map, opt},
    sequence::tuple,
    IResult,
};

#[cfg(feature = "arbitrary")]
use crate::{
    fuzz_eq::FuzzEq,
};
#[cfg(feature = "tracing")]
use crate::utils::bytes_to_display_string;
use crate::print::{Print, Formatter, ToStringFromPrint};
use crate::imf::mailbox;
use crate::text::{ascii, whitespace};

#[derive(Debug, Clone, PartialEq, ToStatic, ToStringFromPrint)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary, FuzzEq))]
pub struct ReturnPath<'a>(pub Option<mailbox::AddrSpec<'a>>);

impl<'a> Print for ReturnPath<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        match &self.0 {
            Some(a) => {
                fmt.write_bytes(b"<");
                a.print(fmt);
                fmt.write_bytes(b">");
            },
            None => fmt.write_bytes(b"<>"),
        }
    }
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = bytes_to_display_string(input)))
)]
pub fn return_path(input: &[u8]) -> IResult<&[u8], ReturnPath<'_>> {
    alt((
        map(mailbox::angle_addr, |a| ReturnPath(Some(a))),
        map(consumed(mailbox::addr_spec), |(_i, a)| {
            // This is not allowed by the RFC but happens in real-world emails
            #[cfg(feature = "tracing-recover")]
            warn!(input = bytes_to_display_string(_i), "bare addr-spec in return-path");
            ReturnPath(Some(a))
        }),
        empty_path
    ))(input)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = bytes_to_display_string(input)))
)]
fn empty_path(input: &[u8]) -> IResult<&[u8], ReturnPath<'_>> {
    let (input, _) = tuple((
        opt(whitespace::cfws),
        tag(&[ascii::LT]),
        opt(whitespace::cfws),
        tag(&[ascii::GT]),
        opt(whitespace::cfws),
    ))(input)?;
    Ok((input, ReturnPath(None)))
}

#[cfg(test)]
mod tests {

    // Return-Path: foo@example.com
    // Return-Path: redundant IMF field
    // - 20150304-What will you be able to say you learned this week-2426.eml
}
