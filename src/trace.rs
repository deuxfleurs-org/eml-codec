use std::collections::HashMap;
use nom::{
    IResult,
    branch::alt,
    bytes::complete::tag,
    character::complete::space0,
    combinator::{map, not, opt, recognize},
    multi::{fold_many0, many0, many1},
    sequence::{delimited, preceded, terminated, pair, tuple},
};
use crate::{address, common_fields, identification, mailbox, model, misc_token, whitespace};

enum RestField<'a> {
    // 3.6.6.  Resent Fields
    ResentDate(model::HeaderDate),
    ResentFrom(Vec<model::MailboxRef>),
    ResentSender(model::MailboxRef),
    ResentTo(Vec<model::AddressRef>),
    ResentCc(Vec<model::AddressRef>),
    ResentBcc(Vec<model::AddressRef>),
    ResentMessageID(model::MessageId<'a>),

    // 3.6.8. Optional fields
    OptionalField(&'a str, String),
}

enum PreludeField {
    // 3.6.7.  Trace Fields
    ReturnPath(String),
    Received(Vec<String>),
}

/// Section
///
/// Optional fields are allowed everywhere in this implementation...
///
/// ```abnf
///*(trace
///   *(optional-field /
///     resent-date /
///     resent-from /
///     resent-sender /
///     resent-to /
///     resent-cc /
///     resent-bcc /
///     resent-msg-id))
/// ```
pub fn section(input: &str) -> IResult<&str, model::Trace> {
    let (input, (path, recv)) = prelude(input)?;
    let (input, mut full_trace) = fold_many0(
        alt((resent_field, unknown_field)),
        model::Trace::default,
        |mut trace, field| {
            match field {
                RestField::ResentDate(date)  => {
                    trace.resent_date = date;
                } 
                RestField::ResentFrom(from)  => {
                    trace.resent_from = from;
                } 
                RestField::ResentSender(sender)  => {
                    trace.resent_sender = Some(sender);
                } 
                RestField::ResentTo(to)  => {
                    trace.resent_to = to;
                } 
                RestField::ResentCc(cc)  => {
                    trace.resent_cc = cc;
                } 
                RestField::ResentBcc(bcc)  => {
                    trace.resent_bcc = bcc;
                } 
                RestField::ResentMessageID(mid)  => {
                    trace.resent_msg_id = Some(mid);
                } 
                RestField::OptionalField(name, body) => {
                    trace.optional.insert(name, body);
                } 
            };
            trace
        }
    )(input)?;
    full_trace.received = recv;
    full_trace.return_path = path;

    Ok((input, full_trace))
}

/// Trace prelude
///
/// ```abnf
/// trace           =   [return]
///                     1*received
/// return          =   "Return-Path:" path CRLF
/// path            =   angle-addr / ([CFWS] "<" [CFWS] ">" [CFWS])
/// received        =   "Received:" *received-token ";" date-time CRLF
/// received-token  =   word / angle-addr / addr-spec / domain
/// ```
fn prelude(input: &str) -> IResult<&str, (Option<model::MailboxRef>, Vec<&str>)> {
    let (input, (return_path, received)) = pair(
        opt(return_path_field), 
        many1(received_field),
    )(input)?;

    Ok((input, (return_path.flatten(), received)))
}

fn return_path_field(input: &str) -> IResult<&str, Option<model::MailboxRef>> {
    delimited(
        pair(tag("Return-Path:"), space0), 
        path,
        whitespace::perm_crlf,
    )(input)
}

fn path(input: &str) -> IResult<&str, Option<model::MailboxRef>> {
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

fn received_field(input: &str) -> IResult<&str, &str> {
    let (input, (_, tk, _, _, _)) = tuple((
        pair(tag("Received:"), space0), 
        recognize(many0(received_tokens)),
        tag(";"),
        common_fields::datetime,
        whitespace::perm_crlf,
    ))(input)?;

    Ok((input, tk))
}

fn received_tokens(input: &str) -> IResult<&str, &str> {
    alt((
        recognize(mailbox::angle_addr),
        recognize(mailbox::addr_spec),
        recognize(mailbox::domain_part),
        recognize(misc_token::word), 
    ))(input)
}

fn resent_field(input: &str) -> IResult<&str, RestField> {
    terminated(
        alt((
            resent_date,
            resent_from,
            resent_sender,
            resent_to,
            resent_cc,
            resent_bcc,
            resent_msg_id,
        )),
        whitespace::perm_crlf,
    )(input)
}

fn resent_date(input: &str) -> IResult<&str, RestField> {
    let (input, body) = preceded(pair(tag("Resent-Date:"), space0), common_fields::datetime)(input)?;
    Ok((input, RestField::ResentDate(body)))
}

fn resent_from(input: &str) -> IResult<&str, RestField> {
    let (input, body) = preceded(pair(tag("Resent-From:"), space0), address::mailbox_list)(input)?;
    Ok((input, RestField::ResentFrom(body)))
}

fn resent_sender(input: &str) -> IResult<&str, RestField> {
    let (input, body) = preceded(pair(tag("Resent-Sender:"), space0), mailbox::mailbox)(input)?;
    Ok((input, RestField::ResentSender(body)))
}

fn resent_to(input: &str) -> IResult<&str, RestField> {
    let (input, body) = preceded(pair(tag("Resent-To:"), space0), address::address_list)(input)?;
    Ok((input, RestField::ResentTo(body)))
}

fn resent_cc(input: &str) -> IResult<&str, RestField> {
    let (input, body) = preceded(pair(tag("Resent-Cc:"), space0), address::address_list)(input)?;
    Ok((input, RestField::ResentCc(body)))
}

fn resent_bcc(input: &str) -> IResult<&str, RestField> {
    let (input, body) = preceded(
        pair(tag("Resent-Bcc:"), space0), 
        opt(alt((address::address_list, address::address_list_cfws))),
    )(input)?;

    Ok((input, RestField::ResentBcc(body.unwrap_or(vec![]))))
}

fn resent_msg_id(input: &str) -> IResult<&str, RestField> {
    let (input, body) = preceded(pair(tag("Resent-Message-ID:"), space0), identification::msg_id)(input)?;
    Ok((input, RestField::ResentMessageID(body)))
}

fn unknown_field(input: &str) -> IResult<&str, RestField> {
    // Check that we:
    //   1. do not start a new trace
    //   2. do not start the common fields
    not(prelude)(input)?;
    not(common_fields::header_field)(input)?;

    // Extract field name
    let (input, field_name) = common_fields::field_name(input)?;
    let (input, body) = misc_token::unstructured(input)?;
    let (input, _) = whitespace::perm_crlf(input)?;
    Ok((input, RestField::OptionalField(field_name, body)))
} 

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, TimeZone};

    #[test]
    fn test_section() {
        let hdrs = r#"Return-Path: <gitlab@example.com>
Received: from smtp.example.com ([10.83.2.2])
    by server with LMTP
    id xxxxxxxxx
    (envelope-from <gitlab@example.com>)
    for <me@example.com>; Tue, 13 Jun 2023 19:01:08 +0000
Resent-Date: Tue, 13 Jun 2023 21:01:07 +0200
Resent-From: <you@example.com>
Resent-Sender: you@example.com
X-Specific: XOXO
Resent-To: Annah <annah@example.com>
Resent-Cc: Empty:;
Resent-Bcc: 
Resent-Message-ID: <note_1985938@example.com>
"#;
        assert_eq!(
            section(hdrs),
            Ok(("", model::Trace {
                return_path: Some(model::MailboxRef { 
                    name: None, 
                    addrspec: model::AddrSpec {
                        local_part: "gitlab".into(),
                        domain: "example.com".into(),
                    }
                }),
                received: vec![
                    r#"from smtp.example.com ([10.83.2.2])
    by server with LMTP
    id xxxxxxxxx
    (envelope-from <gitlab@example.com>)
    for <me@example.com>"#,
                ],

                resent_date: model::HeaderDate::Parsed(
                    FixedOffset::east_opt(2 * 3600)
                    .unwrap()
                    .with_ymd_and_hms(2023, 06, 13, 21, 1, 7)
                    .unwrap()),

                resent_from: vec![
                    model::MailboxRef {
                        name: None,
                        addrspec: model::AddrSpec {
                            local_part: "you".into(),
                            domain: "example.com".into(),
                        }
                    }
                ],
                
                resent_sender: Some(model::MailboxRef {
                    name: None,
                    addrspec: model::AddrSpec {
                        local_part: "you".into(),
                        domain: "example.com".into(),
                    }
                }),

                resent_to: vec![
                    model::AddressRef::Single(model::MailboxRef {
                        name: Some("Annah".into()),
                        addrspec: model::AddrSpec {
                            local_part: "annah".into(),
                            domain: "example.com".into(),
                        }
                    })
                ],

                resent_cc: vec![
                    model::AddressRef::Many(model::GroupRef {
                        name: "Empty".into(),
                        participants: vec![],
                    })
                ],

                resent_bcc: vec![],

                resent_msg_id: Some(model::MessageId {
                    left: "note_1985938",
                    right: "example.com",
                }),

                optional: HashMap::from([("X-Specific", "XOXO".into())]),
            }))
        );
    }
}
