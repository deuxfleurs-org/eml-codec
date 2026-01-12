use bounded_static::ToStatic;
use nom::{
    branch::alt,
    bytes::complete::{is_a, tag},
    combinator::{map, not, opt},
    multi::many0,
    sequence::{terminated, tuple},
    IResult,
};

use crate::print::{print_seq, Print, Formatter};
use crate::imf::{datetime, mailbox};
use crate::text::{ascii, misc_token, whitespace};

#[derive(Debug, PartialEq, ToStatic)]
pub enum ReceivedLogToken<'a> {
    Addr(mailbox::AddrSpec<'a>),
    Domain(mailbox::Domain<'a>),
    Word(misc_token::Word<'a>),
}

impl<'a> Print for ReceivedLogToken<'a> {
    fn print(&self, fmt: &mut impl Formatter) -> std::io::Result<()> {
        match self {
            ReceivedLogToken::Addr(a) => a.print(fmt),
            ReceivedLogToken::Domain(d) => d.print(fmt),
            ReceivedLogToken::Word(w) => w.print(fmt),
        }
    }
}

#[derive(Debug, PartialEq, ToStatic)]
pub struct ReceivedLog<'a> {
    pub log: Vec<ReceivedLogToken<'a>>,
    pub date: datetime::DateTime,
}

impl<'a> Print for ReceivedLog<'a> {
    fn print(&self, fmt: &mut impl Formatter) -> std::io::Result<()> {
        print_seq(fmt, &self.log, Formatter::write_fws)?;
        fmt.write_bytes(b";")?;
        fmt.write_fws()?;
        self.date.print(fmt)
    }
}

#[derive(Debug, Clone, PartialEq, ToStatic)]
pub struct ReturnPath<'a>(pub Option<mailbox::AddrSpec<'a>>);

impl<'a> Print for ReturnPath<'a> {
    fn print(&self, fmt: &mut impl Formatter) -> std::io::Result<()> {
        match &self.0 {
            Some(a) => a.print(fmt),
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
        tuple((many0(received_tokens), tag(";"), datetime::date_time)),
        |(tokens, _, dt)| ReceivedLog {
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
    use chrono::{FixedOffset, TimeZone};

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
                        ReceivedLogToken::Word(Word::Atom(b"from"[..].into())),
                        ReceivedLogToken::Domain(mailbox::Domain::Atoms(vec![
                            b"smtp"[..].into(),
                            b"example"[..].into(),
                            b"com"[..].into(),
                        ])),
                        ReceivedLogToken::Word(Word::Atom(b"by"[..].into())),
                        ReceivedLogToken::Word(Word::Atom(b"server"[..].into())),
                        ReceivedLogToken::Word(Word::Atom(b"with"[..].into())),
                        ReceivedLogToken::Word(Word::Atom(b"LMTP"[..].into())),
                        ReceivedLogToken::Word(Word::Atom(b"id"[..].into())),
                        ReceivedLogToken::Word(Word::Atom(b"xxxxxxxxx"[..].into())),
                        ReceivedLogToken::Word(Word::Atom(b"for"[..].into())),
                        ReceivedLogToken::Addr(mailbox::AddrSpec {
                            local_part: mailbox::LocalPart(vec![mailbox::LocalPartToken::Word(
                                Word::Atom(b"me"[..].into())
                            )]),
                            domain: mailbox::Domain::Atoms(vec![
                                b"example"[..].into(),
                                b"com"[..].into(),
                            ]),
                        })
                    ],
                }
            ))
        );
    }
}
