#[cfg(feature = "arbitrary")]
use arbitrary::Arbitrary;
use bounded_static::ToStatic;
use nom::{
    branch::alt,
    bytes::complete::{is_a, tag},
    combinator::{map, not, opt},
    multi::many0,
    sequence::{terminated, tuple},
    IResult,
};

#[cfg(feature = "arbitrary")]
use crate::{
    arbitrary_utils::arbitrary_vec_nonempty,
    fuzz_eq::FuzzEq,
};
use crate::print::{print_seq, Print, Formatter};
use crate::imf::{datetime, mailbox};
use crate::text::{ascii, misc_token, whitespace};

// Invariant: only the first block may have `return_path` set to `None`.
#[derive(Clone, Debug, Default, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub struct Trace<'a>(pub Vec<TraceBlock<'a>>);

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for Trace<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let blocks: Vec<TraceBlock> = u.arbitrary()?;
        if blocks.len() > 0 {
            if blocks[1..].iter().any(|b| b.return_path.is_none()) {
                return Err(arbitrary::Error::IncorrectFormat)
            }
        }
        Ok(Trace(blocks))
    }
}

#[derive(Clone, Debug, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub struct TraceBlock<'a> {
    pub return_path: Option<ReturnPath<'a>>,
    pub received: Vec<ReceivedLog<'a>>, // non-empty
}

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for TraceBlock<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(TraceBlock{
            return_path: u.arbitrary()?,
            received: arbitrary_vec_nonempty(u)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
pub enum ReceivedLogToken<'a> {
    Addr(mailbox::AddrSpec<'a>),
    Domain(mailbox::Domain<'a>),
    Word(misc_token::Word<'a>),
}

impl<'a> Print for ReceivedLogToken<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        match self {
            ReceivedLogToken::Addr(a) => a.print(fmt),
            ReceivedLogToken::Domain(d) => d.print(fmt),
            ReceivedLogToken::Word(w) => w.print(fmt),
        }
    }
}

// Domain and Word overlap; implement a somewhat sloppy equality
#[cfg(feature = "arbitrary")]
impl<'a> FuzzEq for ReceivedLogToken<'a> {
    fn fuzz_eq(&self, other: &Self) -> bool {
        crate::print::with_formatter(None, |fmt| self.print(fmt)) ==
        crate::print::with_formatter(None, |fmt| other.print(fmt))
    }
}

#[derive(Clone, Debug, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary, FuzzEq))]
pub struct ReceivedLog<'a> {
    pub log: Vec<ReceivedLogToken<'a>>,
    pub date: datetime::DateTime,
}

impl<'a> Print for ReceivedLog<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        print_seq(fmt, &self.log, Formatter::write_fws);
        fmt.write_bytes(b";");
        fmt.write_fws();
        self.date.print(fmt)
    }
}

#[derive(Debug, Clone, PartialEq, ToStatic)]
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

/*
impl<'a> TryFrom<&'a lazy::ReceivedLog<'a>> for ReceivedLog<'a> {
    type Error = IMFError<'a>;

    fn try_from(input: &'a lazy::ReceivedLog<'a>) -> Result<Self, Self::Error> {
        received_body(input.0)
            .map_err(|e| IMFError::ReceivedLog(e))
            .map(|(_, v)| ReceivedLog(v))
    }
}*/

pub fn received_log(input: &[u8]) -> IResult<&[u8], ReceivedLog<'_>> {
    map(
        tuple((many0(received_tokens), opt(whitespace::cfws), tag(";"), datetime::date_time)),
        |(tokens, _, _, dt)| ReceivedLog {
            log: tokens,
            date: dt,
        },
    )(input)
}

pub fn return_path(input: &[u8]) -> IResult<&[u8], ReturnPath<'_>> {
    alt((
        map(mailbox::angle_addr, |a| ReturnPath(Some(a))),
        empty_path
    ))(input)
}

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

fn received_tokens(input: &[u8]) -> IResult<&[u8], ReceivedLogToken<'_>> {
    alt((
        terminated(
            map(misc_token::word, ReceivedLogToken::Word),
            not(is_a([ascii::PERIOD, ascii::AT])),
        ),
        map(mailbox::angle_addr, ReceivedLogToken::Addr),
        map(mailbox::addr_spec, ReceivedLogToken::Addr),
        map(mailbox::obs_domain, ReceivedLogToken::Domain),
    ))(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::imf::trace::misc_token::Word;
    use crate::text::words::Atom;
    use chrono::{FixedOffset, TimeZone};

    #[test]
    fn test_received_body_no_log() {
        assert_eq!(
            received_log(b" ; 31 Dec 1969 00:01:00 +0000"),
            Ok((
                &b""[..],
                ReceivedLog {
                    date:
                    datetime::DateTime(
                        FixedOffset::east_opt(0)
                            .unwrap()
                            .with_ymd_and_hms(1969, 12, 31, 0, 1, 0)
                            .unwrap()
                    ),
                    log: vec![],
                }
            ))
        )
    }

    #[test]
    fn test_received_body() {
        let hdrs = r#"from smtp.example.com ([10.83.2.2])
    by server with LMTP
    id xxxxxxxxx
    (envelope-from <gitlab@example.com>)
    for <me@example.com>; Tue, 13 Jun 2023 19:01:08 +0000"#
            .as_bytes();

        assert_eq!(
            received_log(hdrs),
            Ok((
                &b""[..],
                ReceivedLog {
                    date:
                    datetime::DateTime(
                        FixedOffset::east_opt(0)
                            .unwrap()
                            .with_ymd_and_hms(2023, 06, 13, 19, 1, 8)
                            .unwrap()
                    ),
                    log: vec![
                        ReceivedLogToken::Word(Word::Atom(Atom(b"from"[..].into()))),
                        ReceivedLogToken::Domain(mailbox::Domain::Atoms(vec![
                            Atom(b"smtp"[..].into()),
                            Atom(b"example"[..].into()),
                            Atom(b"com"[..].into()),
                        ])),
                        ReceivedLogToken::Word(Word::Atom(Atom(b"by"[..].into()))),
                        ReceivedLogToken::Word(Word::Atom(Atom(b"server"[..].into()))),
                        ReceivedLogToken::Word(Word::Atom(Atom(b"with"[..].into()))),
                        ReceivedLogToken::Word(Word::Atom(Atom(b"LMTP"[..].into()))),
                        ReceivedLogToken::Word(Word::Atom(Atom(b"id"[..].into()))),
                        ReceivedLogToken::Word(Word::Atom(Atom(b"xxxxxxxxx"[..].into()))),
                        ReceivedLogToken::Word(Word::Atom(Atom(b"for"[..].into()))),
                        ReceivedLogToken::Addr(mailbox::AddrSpec {
                            local_part: mailbox::LocalPart(vec![mailbox::LocalPartToken::Word(
                                Word::Atom(Atom(b"me"[..].into()))
                            )]),
                            domain: mailbox::Domain::Atoms(vec![
                                Atom(b"example"[..].into()),
                                Atom(b"com"[..].into()),
                            ]),
                        })
                    ],
                }
            ))
        );
    }
}
