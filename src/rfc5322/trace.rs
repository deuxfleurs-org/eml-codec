use nom::{
    branch::alt,
    bytes::complete::{is_a, tag},
    combinator::{map, opt, not},
    multi::many0,
    sequence::{tuple, terminated},
    IResult,
};
use chrono::{DateTime, FixedOffset};

use crate::rfc5322::{datetime, mailbox};
use crate::text::{ascii, whitespace, misc_token };

#[derive(Debug, PartialEq)]
pub enum ReceivedLogToken<'a> {
    Addr(mailbox::AddrSpec<'a>),
    Domain(mailbox::Domain<'a>),
    Word(misc_token::Word<'a>)
}

#[derive(Debug, PartialEq)]
pub struct ReceivedLog<'a> {
    pub log: Vec<ReceivedLogToken<'a>>,
    pub date: Option<DateTime<FixedOffset>>,
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

pub fn received_log(input: &[u8]) -> IResult<&[u8], ReceivedLog> {
    map(
        tuple((
            many0(received_tokens),
            tag(";"),
            datetime::section,
        )),
        |(tokens, _, dt)| ReceivedLog { log: tokens, date: dt } ,
    )(input)
}

pub fn return_path(input: &[u8]) -> IResult<&[u8], Option<mailbox::AddrSpec>> {
    alt((map(mailbox::angle_addr, |a| Some(a)), empty_path))(input)
}

fn empty_path(input: &[u8]) -> IResult<&[u8], Option<mailbox::AddrSpec>> {
    let (input, _) = tuple((
        opt(whitespace::cfws),
        tag(&[ascii::LT]),
        opt(whitespace::cfws),
        tag(&[ascii::GT]),
        opt(whitespace::cfws),
    ))(input)?;
    Ok((input, None))
}

fn received_tokens(input: &[u8]) -> IResult<&[u8], ReceivedLogToken> {
    alt((
        terminated(map(misc_token::word, |x| ReceivedLogToken::Word(x)), not(is_a([ascii::PERIOD, ascii::AT]))),
        map(mailbox::angle_addr, |x| ReceivedLogToken::Addr(x)),
        map(mailbox::addr_spec, |x| ReceivedLogToken::Addr(x)),
        map(mailbox::obs_domain, |x| ReceivedLogToken::Domain(x)),
    ))(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use crate::rfc5322::trace::misc_token::Word;

    #[test]
    fn test_received_body() {
        let hdrs = r#"from smtp.example.com ([10.83.2.2])
    by server with LMTP
    id xxxxxxxxx
    (envelope-from <gitlab@example.com>)
    for <me@example.com>; Tue, 13 Jun 2023 19:01:08 +0000"#.as_bytes();

        assert_eq!(
            received_body(hdrs),
            Ok((
                &b""[..],
                ReceivedLog {
                    date: Some(FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(2023, 06, 13, 19, 1, 8).unwrap()),
                    log: vec![
                        ReceivedLogToken::Word(Word::Atom(&b"from"[..])),
                        ReceivedLogToken::Domain(mailbox::Domain::Atoms(vec![&b"smtp"[..], &b"example"[..], &b"com"[..]])),
                        ReceivedLogToken::Word(Word::Atom(&b"by"[..])),
                        ReceivedLogToken::Word(Word::Atom(&b"server"[..])),
                        ReceivedLogToken::Word(Word::Atom(&b"with"[..])),
                        ReceivedLogToken::Word(Word::Atom(&b"LMTP"[..])),
                        ReceivedLogToken::Word(Word::Atom(&b"id"[..])),
                        ReceivedLogToken::Word(Word::Atom(&b"xxxxxxxxx"[..])),
                        ReceivedLogToken::Word(Word::Atom(&b"for"[..])),
                        ReceivedLogToken::Addr(mailbox::AddrSpec {
                            local_part: mailbox::LocalPart(vec![mailbox::LocalPartToken::Word(Word::Atom(&b"me"[..]))]),
                            domain: mailbox::Domain::Atoms(vec![&b"example"[..], &b"com"[..]]), 
                        })
                    ],
                }   
            ))
        );
    }
}
