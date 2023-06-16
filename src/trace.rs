use std::collections::HashMap;
use nom::{
    IResult,
    branch::alt,
    bytes::complete::tag,
    character::complete::space0,
    combinator::{map, opt, recognize},
    multi::many0,
    sequence::{delimited, pair, tuple},
};
use crate::{datetime, mailbox, model, misc_token, whitespace};

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
    alt((
        map(mailbox::angle_addr, |a| Some(a)), 
        empty_path
    ))(input)
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

fn received_tokens(input: &str) -> IResult<&str, &str> {
    alt((
        recognize(mailbox::angle_addr),
        recognize(mailbox::addr_spec),
        recognize(mailbox::domain_part),
        recognize(misc_token::word), 
    ))(input)
}


#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, TimeZone};

    #[test]
    fn test_received() {
        let hdrs = r#"from smtp.example.com ([10.83.2.2])
    by server with LMTP
    id xxxxxxxxx
    (envelope-from <gitlab@example.com>)
    for <me@example.com>; Tue, 13 Jun 2023 19:01:08 +0000"#;

        assert_eq!(
            received_body(hdrs),
            Ok(("",  r#"from smtp.example.com ([10.83.2.2])
    by server with LMTP
    id xxxxxxxxx
    (envelope-from <gitlab@example.com>)
    for <me@example.com>"#))
        );
    }
}
