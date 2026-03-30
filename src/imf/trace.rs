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
        map(consumed(mailbox::mailbox), |(_i, m)| {
            // This is not allowed by the RFC but happens in some real-world emails
            #[cfg(feature = "tracing-recover")]
            warn!(input = bytes_to_display_string(_i), "mailbox in return-path");
            ReturnPath(Some(m.addrspec))
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
    use super::*;
    use crate::imf::mailbox::*;
    use crate::text::words::Atom;
    use crate::text::misc_token::Word;

    #[test]
    fn test_return_path() {
        assert_eq!(
            return_path(b" <foo@example.com>"),
            Ok((
                &b""[..],
                ReturnPath(Some(AddrSpec {
                    local_part: LocalPart(vec![
                        LocalPartToken::Word(Word::Atom(Atom("foo"[..].into())))
                    ]),
                    domain: Domain::Atoms(vec![Atom("example"[..].into()), Atom("com"[..].into())]),
                }))
            ))
        );
    }

    #[test]
    fn test_return_path_bare() {
        assert_eq!(
            return_path(b" foo@example.com "),
            Ok((
                &b""[..],
                ReturnPath(Some(AddrSpec {
                    local_part: LocalPart(vec![
                        LocalPartToken::Word(Word::Atom(Atom("foo"[..].into())))
                    ]),
                    domain: Domain::Atoms(vec![Atom("example"[..].into()), Atom("com"[..].into())]),
                }))
            ))
        );
    }

    #[test]
    fn test_return_path_mailbox() {
        assert_eq!(
            return_path(b"abcdef <foo@example.com> "),
            Ok((
                &b""[..],
                ReturnPath(Some(AddrSpec {
                    local_part: LocalPart(vec![
                        LocalPartToken::Word(Word::Atom(Atom("foo"[..].into())))
                    ]),
                    domain: Domain::Atoms(vec![Atom("example"[..].into()), Atom("com"[..].into())]),
                }))
            ))
        );
    }
}
