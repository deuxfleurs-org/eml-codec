#[cfg(feature = "arbitrary")]
use arbitrary::Arbitrary;
#[cfg(feature = "tracing")]
use tracing::warn;
use bounded_static::ToStatic;
use nom::{
    branch::alt,
    bytes::complete::{is_a, tag},
    combinator::{consumed, map, not, opt},
    multi::many0,
    sequence::{terminated, tuple},
    IResult,
};

#[cfg(feature = "arbitrary")]
use crate::{
    fuzz_eq::FuzzEq,
};
#[cfg(feature = "tracing")]
use crate::utils::bytes_to_display_string;
use crate::print::{print_seq, Print, Formatter, ToStringFromPrint};
use crate::imf::{datetime, mailbox};
use crate::text::{ascii, misc_token, whitespace};

#[derive(Clone, Debug, PartialEq, ToStatic, ToStringFromPrint)]
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
        self.to_string() == other.to_string()
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

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = bytes_to_display_string(input)))
)]
pub fn received_log(input: &[u8]) -> IResult<&[u8], ReceivedLog<'_>> {
    map(
        tuple((many0(received_token), opt(whitespace::cfws), tag(";"), datetime::date_time)),
        |(tokens, _, _, dt)| ReceivedLog {
            log: tokens,
            date: dt,
        },
    )(input)
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

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = bytes_to_display_string(input)))
)]
fn received_token(input: &[u8]) -> IResult<&[u8], ReceivedLogToken<'_>> {
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
    fn test_received_token() {
        assert_eq!(
            received_token(b"from smtp.example.com"),
            Ok((&b"smtp.example.com"[..],
                ReceivedLogToken::Word(Word::Atom(Atom("from"[..].into())))
                ))
        );

        assert_eq!(
            received_token(b"smtp.example.com"),
            Ok((&b""[..],
                ReceivedLogToken::Domain(mailbox::Domain::Atoms(vec![
                    Atom("smtp"[..].into()),
                    Atom("example"[..].into()),
                    Atom("com"[..].into()),
                ]))
            ))
        );
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
                        ReceivedLogToken::Word(Word::Atom(Atom("from"[..].into()))),
                        ReceivedLogToken::Domain(mailbox::Domain::Atoms(vec![
                            Atom("smtp"[..].into()),
                            Atom("example"[..].into()),
                            Atom("com"[..].into()),
                        ])),
                        ReceivedLogToken::Word(Word::Atom(Atom("by"[..].into()))),
                        ReceivedLogToken::Word(Word::Atom(Atom("server"[..].into()))),
                        ReceivedLogToken::Word(Word::Atom(Atom("with"[..].into()))),
                        ReceivedLogToken::Word(Word::Atom(Atom("LMTP"[..].into()))),
                        ReceivedLogToken::Word(Word::Atom(Atom("id"[..].into()))),
                        ReceivedLogToken::Word(Word::Atom(Atom("xxxxxxxxx"[..].into()))),
                        ReceivedLogToken::Word(Word::Atom(Atom("for"[..].into()))),
                        ReceivedLogToken::Addr(mailbox::AddrSpec {
                            local_part: mailbox::LocalPart(vec![mailbox::LocalPartToken::Word(
                                Word::Atom(Atom("me"[..].into()))
                            )]),
                            domain: mailbox::Domain::Atoms(vec![
                                Atom("example"[..].into()),
                                Atom("com"[..].into()),
                            ]),
                        })
                    ],
                }
            ))
        );
    }

    #[test]
    fn test_received_log2() {
        let hdrs = r#"X.X.X Y foo@bar.com; Tue, 13 Jun 2023 19:01:08 +0000"#
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
                        ReceivedLogToken::Domain(mailbox::Domain::Atoms(vec![
                            Atom("X"[..].into()),
                            Atom("X"[..].into()),
                            Atom("X"[..].into()),
                        ])),
                        ReceivedLogToken::Word(Word::Atom(Atom("Y"[..].into()))),
                        ReceivedLogToken::Addr(mailbox::AddrSpec {
                            local_part: mailbox::LocalPart(vec![mailbox::LocalPartToken::Word(
                                Word::Atom(Atom("foo"[..].into()))
                            )]),
                            domain: mailbox::Domain::Atoms(vec![
                                Atom("bar"[..].into()),
                                Atom("com"[..].into()),
                            ]),
                        })
                    ],
                }
            ))
        );
    }

    // Return-Path: foo@example.com
    // Return-Path: redundant IMF field
    // - 20150304-What will you be able to say you learned this week-2426.eml

    // Received: by filter0237p1iad2.sendgrid.net with SMTP id filter0237p1iad2-16618-5C145383-3B\r\n        2018-12-15 01:06:11.453484092 +0000 UTC m=+95790.695923185
    // Received: from NTQ3Njk (35.52.148.146.bc.googleusercontent.com [146.148.52.35])\r\n\tby ismtpd0026p1iad2.sendgrid.net (SG) with HTTP id LjpG4NJ9R5ySPVYIUeq6fQ\r\n\tSat, 15 Dec 2018 01:06:11.387 +0000 (UTC)
    // Received: redundant IMF field
    // - 20150304-What will you be able to say you learned this week-2426.eml

}
