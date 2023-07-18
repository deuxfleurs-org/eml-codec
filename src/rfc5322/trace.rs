use crate::error::IMFError;
use crate::fragments::{datetime, lazy, mailbox, misc_token, model, whitespace};
use nom::{
    branch::alt,
    bytes::complete::tag,
    combinator::{map, opt, recognize},
    multi::many0,
    sequence::tuple,
    IResult,
};

#[derive(Debug, PartialEq)]
pub struct ReceivedLog<'a>(pub &'a str);

impl<'a> TryFrom<&'a lazy::ReceivedLog<'a>> for ReceivedLog<'a> {
    type Error = IMFError<'a>;

    fn try_from(input: &'a lazy::ReceivedLog<'a>) -> Result<Self, Self::Error> {
        received_body(input.0)
            .map_err(|e| IMFError::ReceivedLog(e))
            .map(|(_, v)| ReceivedLog(v))
    }
}

pub fn received_body(input: &str) -> IResult<&str, &str> {
    map(
        tuple((
            recognize(many0(received_tokens)),
            tag(";"),
            datetime::section,
        )),
        |(tokens, _, _)| tokens,
    )(input)
}

pub fn return_path_body(input: &str) -> IResult<&str, Option<model::MailboxRef>> {
    alt((map(mailbox::angle_addr, |a| Some(a)), empty_path))(input)
}

fn empty_path(input: &str) -> IResult<&str, Option<model::MailboxRef>> {
    let (input, _) = tuple((
        opt(whitespace::cfws),
        tag("<"),
        opt(whitespace::cfws),
        tag(">"),
        opt(whitespace::cfws),
    ))(input)?;
    Ok((input, None))
}

// @FIXME use obs_domain as it is a superset of domain
fn received_tokens(input: &str) -> IResult<&str, &str> {
    alt((
        recognize(mailbox::angle_addr),
        recognize(mailbox::addr_spec),
        recognize(mailbox::obs_domain),
        recognize(misc_token::word),
    ))(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_received_body() {
        let hdrs = r#"from smtp.example.com ([10.83.2.2])
    by server with LMTP
    id xxxxxxxxx
    (envelope-from <gitlab@example.com>)
    for <me@example.com>; Tue, 13 Jun 2023 19:01:08 +0000"#;

        assert_eq!(
            received_body(hdrs),
            Ok((
                "",
                r#"from smtp.example.com ([10.83.2.2])
    by server with LMTP
    id xxxxxxxxx
    (envelope-from <gitlab@example.com>)
    for <me@example.com>"#
            ))
        );
    }
}
