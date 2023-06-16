use nom::{
    IResult,
    branch::alt,
    bytes::complete::take_while1,
    bytes::complete::tag,
    character::complete::space0,
    combinator::{map, opt},
    multi::{many0, many1, fold_many0, separated_list1},
    sequence::{terminated, preceded, pair, tuple},
};

use crate::whitespace::{fws, perm_crlf};
use crate::words::vchar_seq;
use crate::misc_token::{phrase, unstructured};
use crate::model::{HeaderSection, HeaderDate, MailboxRef, AddressRef};
use crate::mailbox::mailbox;
use crate::address::{mailbox_list, address_list, address_list_cfws};
use crate::identification::msg_id;
use crate::{datetime, trace, model};

/// HEADERS

/// Header section
///
/// See: https://www.rfc-editor.org/rfc/rfc5322.html#section-2.2
pub fn section(input: &str) -> IResult<&str, HeaderSection> {
    let (input, headers) = fold_many0(
        alt((header_field, unknown_field)),
        HeaderSection::default,
        |mut section, head| {
            match head {
                //@FIXME min and max limits are not enforced,
                // it may result in missing data or silently overriden data.

                // 3.6.1.  The Origination Date Field
                HeaderField::Date(d) => {
                    //   | orig-date      | 1      | 1          |                            |
                    section.date = d;
                }

                // 3.6.2.  Originator Fields
                HeaderField::From(v) => {
                    //   | from           | 1      | 1          | See sender and 3.6.2       |
                    section.from = v;
                }
                HeaderField::Sender(mbx) => {
                    //   | sender         | 0*     | 1          | MUST occur with  multi-address from - see 3.6.2 |
                    section.sender = Some(mbx);
                }
                HeaderField::ReplyTo(addr_list) => {
                    //   | reply-to       | 0      | 1          |                            |
                    section.reply_to = addr_list;
                }

                // 3.6.3.  Destination Address Fields
                HeaderField::To(addr_list) => {
                    //    | to             | 0      | 1          |                            |
                    section.to = addr_list;
                }
                HeaderField::Cc(addr_list) => {
                    //    | cc             | 0      | 1          |                            |
                    section.cc = addr_list;
                }
                HeaderField::Bcc(addr_list) => {
                    //    | bcc            | 0      | 1          |                            |
                    section.bcc = addr_list;
                }

                // 3.6.4.  Identification Fields
                HeaderField::MessageID(msg_id) => {
                    //    | message-id     | 0*     | 1          | SHOULD be present - see  3.6.4 |
                    section.msg_id = Some(msg_id);
                }
                HeaderField::InReplyTo(id_list) => {
                    //    | in-reply-to    | 0*     | 1          | SHOULD occur in some replies - see 3.6.4  |
                    section.in_reply_to = id_list;
                }
                HeaderField::References(id_list) => {
                    //    | in-reply-to    | 0*     | 1          | SHOULD occur in some replies - see 3.6.4  |
                    section.references = id_list;
                }

                // 3.6.5.  Informational Fields
                HeaderField::Subject(title) => {
                    //    | subject        | 0      | 1          |                            |
                    section.subject = Some(title);
                }               
                HeaderField::Comments(coms) => {
                    //    | comments       | 0      | unlimited  |                            |
                    section.comments.push(coms);
                }
                HeaderField::Keywords(mut kws) => {
                    //    | keywords       | 0      | unlimited  |                            |
                    section.keywords.append(&mut kws);
                }

                // 3.6.6   Resent Fields are not implemented
                // 3.6.7   Trace Fields
                HeaderField::ReturnPath(maybe_mbx) => {
                    if let Some(mbx) = maybe_mbx {
                        section.return_path.push(mbx);
                    }
                }
                HeaderField::Received(log) => {
                    section.received.push(log);
                }

                // 3.6.8.  Optional Fields
                HeaderField::Optional(name, body) => {
                    section.optional.insert(name, body);
                }
            };
            section
        }
    )(input)?;
    
    let (input, _) = perm_crlf(input)?;
    Ok((input, headers))
}

#[derive(Debug, PartialEq)]
pub enum HeaderField<'a> {
    // 3.6.1.  The Origination Date Field
    Date(HeaderDate),

    // 3.6.2.  Originator Fields
    From(Vec<MailboxRef>),
    Sender(MailboxRef),
    ReplyTo(Vec<AddressRef>),

    // 3.6.3.  Destination Address Fields
    To(Vec<AddressRef>),
    Cc(Vec<AddressRef>),
    Bcc(Vec<AddressRef>),

    // 3.6.4.  Identification Fields
    MessageID(model::MessageId<'a>),
    InReplyTo(Vec<model::MessageId<'a>>),
    References(Vec<model::MessageId<'a>>),

    // 3.6.5.  Informational Fields
    Subject(String),
    Comments(String),
    Keywords(Vec<String>),

    // 3.6.6   Resent Fields (not implemented)
    // 3.6.7   Trace Fields
    Received(&'a str),
    ReturnPath(Option<model::MailboxRef>),

    // 3.6.8.  Optional Fields
    Optional(&'a str, String)
}

/// Parse one known header field
///
/// RFC5322 optional-field seems to be a generalization of the field terminology.
/// We use it to parse all header names:
pub fn header_field(input: &str) -> IResult<&str, HeaderField> {
    terminated(
        alt((
            // 3.6.1.  The Origination Date Field
            date,
            // 3.6.2.  Originator Fields
            alt((from, sender, reply_to)),
            // 3.6.3.  Destination Address Fields
            alt((to, cc, bcc)),
            // 3.6.4.  Identification Fields
            alt((msg_id_field, in_reply_to, references)),
            // 3.6.5.  Informational Fields
            alt((subject, comments, keywords)),
            // 3.6.7   Trace field
            alt((return_path, received)),
        )),
        perm_crlf,
    )(input)
}

// 3.6.1.  The Origination Date Field
fn date(input: &str) -> IResult<&str, HeaderField> {
    let (input, body) = preceded(pair(tag("Date:"), space0), datetime::section)(input)?;
    Ok((input, HeaderField::Date(body)))
}

// 3.6.2.  Originator Fields
fn from(input: &str) -> IResult<&str, HeaderField> {
    let (input, body) = preceded(pair(tag("From:"), space0), mailbox_list)(input)?;
    Ok((input, HeaderField::From(body)))
}
fn sender(input: &str) -> IResult<&str, HeaderField> {
    let (input, body) = preceded(pair(tag("Sender:"), space0), mailbox)(input)?;
    Ok((input, HeaderField::Sender(body)))
}
fn reply_to(input: &str) -> IResult<&str, HeaderField> {
    let (input, body) = preceded(pair(tag("Reply-To:"), space0), address_list)(input)?;
    Ok((input, HeaderField::ReplyTo(body)))
}

// 3.6.3.  Destination Address Fields
fn to(input: &str) -> IResult<&str, HeaderField> {
    let (input, body) = preceded(pair(tag("To:"), space0), address_list)(input)?;
    Ok((input, HeaderField::To(body)))
}
fn cc(input: &str) -> IResult<&str, HeaderField> {
    let (input, body) = preceded(pair(tag("Cc:"), space0), address_list)(input)?;
    Ok((input, HeaderField::Cc(body)))
}
fn bcc(input: &str) -> IResult<&str, HeaderField> {
    let (input, body) = preceded(
        pair(tag("Bcc:"), space0), 
        opt(alt((address_list, address_list_cfws))),
    )(input)?;

    Ok((input, HeaderField::Bcc(body.unwrap_or(vec![]))))
}

// 3.6.4.  Identification Fields
fn msg_id_field(input: &str) -> IResult<&str, HeaderField> {
    let (input, body) = preceded(pair(tag("Message-ID:"), space0), msg_id)(input)?;
    Ok((input, HeaderField::MessageID(body)))
}
fn in_reply_to(input: &str) -> IResult<&str, HeaderField> {
    let (input, body) = preceded(pair(tag("In-Reply-To:"), space0), many1(msg_id))(input)?;
    Ok((input, HeaderField::InReplyTo(body)))
}
fn references(input: &str) -> IResult<&str, HeaderField> {
    let (input, body) = preceded(pair(tag("References:"), space0), many1(msg_id))(input)?;
    Ok((input, HeaderField::References(body)))
}

// 3.6.5.  Informational Fields
fn subject(input: &str) -> IResult<&str, HeaderField> {
    let (input, body) = preceded(pair(tag("Subject:"), space0), unstructured)(input)?;
    Ok((input, HeaderField::Subject(body)))
}
fn comments(input: &str) -> IResult<&str, HeaderField> {
    let (input, body) = preceded(pair(tag("Comments:"), space0), unstructured)(input)?;
    Ok((input, HeaderField::Comments(body)))
}
fn keywords(input: &str) -> IResult<&str, HeaderField> {
    let (input, body) = preceded(
        pair(tag("Keywords:"), space0), 
        separated_list1(tag(","), phrase),
    )(input)?;
    Ok((input, HeaderField::Keywords(body)))
}

// 3.6.6 Resent fields
// Not implemented

// 3.6.7 Trace fields
fn return_path(input: &str) -> IResult<&str, HeaderField> {
    map(
        preceded(pair(tag("Return-Path:"), space0), trace::return_path_body),
        |body| HeaderField::ReturnPath(body),
    )(input)
}
fn received(input: &str) -> IResult<&str, HeaderField> {
    map(
        preceded(pair(tag("Received:"), space0), trace::received_body),
        |body| HeaderField::Received(body),
    )(input)
}

/// Optional field
///
/// ```abnf
/// field      =   field-name ":" unstructured CRLF
/// field-name =   1*ftext
/// ftext      =   %d33-57 /          ; Printable US-ASCII
///                %d59-126           ;  characters not including
///                                   ;  ":".
/// ```
fn unknown_field(input: &str) -> IResult<&str, HeaderField> {
    // Extract field name
    let (input, field_name) = field_name(input)?;
    let (input, body) = unstructured(input)?;
    let (input, _) = perm_crlf(input)?;
    Ok((input, HeaderField::Optional(field_name, body)))
}
fn field_name(input: &str) -> IResult<&str, &str> {
    terminated(
        take_while1(|c| c >= '\x21' && c <= '\x7E' && c != '\x3A'),
        pair(tag(":"), space0)
    )(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{GroupRef, AddrSpec};

    // 3.6.1.  The Origination Date Field
    #[test]
    fn test_datetime() {
        let datefield = "Date: Thu,\r\n  13\r\n  Feb\r\n    1969\r\n 23:32\r\n   -0330 (Newfoundland Time)\r\n";
        let (input, v) = header_field(datefield).unwrap();
        assert_eq!(input, "");
        match v {
            HeaderField::Date(HeaderDate::Parsed(_)) => (),
            _ => panic!("Date has not been parsed"),
        };
    }

    // 3.6.2.  Originator Fields
    #[test]
    fn test_from() {
        assert_eq!(
            header_field("From: \"Joe Q. Public\" <john.q.public@example.com>\r\n"),
            Ok(("", HeaderField::From(vec![MailboxRef { 
                name: Some("Joe Q. Public".into()), 
                addrspec: AddrSpec {
                    local_part: "john.q.public".into(),
                    domain: "example.com".into(),
                }
            }]))),
        );
    }
    #[test]
    fn test_sender() {
        assert_eq!(
            header_field("Sender: Michael Jones <mjones@machine.example>\r\n"),
            Ok(("", HeaderField::Sender(MailboxRef {
                name: Some("Michael Jones".into()),
                addrspec: AddrSpec {
                    local_part: "mjones".into(),
                    domain: "machine.example".into(),
                },
            }))),
        );
    }
    #[test]
    fn test_reply_to() {
        assert_eq!(
            header_field("Reply-To: \"Mary Smith: Personal Account\" <smith@home.example>\r\n"),
            Ok(("", HeaderField::ReplyTo(vec![AddressRef::Single(MailboxRef {
                name: Some("Mary Smith: Personal Account".into()),
                addrspec: AddrSpec {
                    local_part: "smith".into(),
                    domain: "home.example".into(),
                },
            })])))
        );
    }

    // 3.6.3.  Destination Address Fields
    #[test]
    fn test_to() {
        assert_eq!(
            header_field("To: A Group:Ed Jones <c@a.test>,joe@where.test,John <jdoe@one.test>;\r\n"),
            Ok(("", HeaderField::To(vec![AddressRef::Many(GroupRef {
                name: "A Group".into(),
                participants: vec![
                    MailboxRef {
                        name: Some("Ed Jones".into()),
                        addrspec: AddrSpec { local_part: "c".into(), domain: "a.test".into() },
                    },
                    MailboxRef {
                        name: None,
                        addrspec: AddrSpec { local_part: "joe".into(), domain: "where.test".into() },
                    },
                    MailboxRef {
                        name: Some("John".into()),
                        addrspec: AddrSpec { local_part: "jdoe".into(), domain: "one.test".into() },
                    },
                ]
            })])))
        );
    }
    #[test]
    fn test_cc() {
        assert_eq!(
            header_field("Cc: Undisclosed recipients:;\r\n"),
            Ok(("", HeaderField::Cc(vec![AddressRef::Many(GroupRef {
                name: "Undisclosed recipients".into(),
                participants: vec![],
            })])))
        );
    }
    #[test]
    fn test_bcc() {
        assert_eq!(
            header_field("Bcc: (empty)\r\n"),
            Ok(("", HeaderField::Bcc(vec![])))
        );
        assert_eq!(
            header_field("Bcc: \r\n"),
            Ok(("", HeaderField::Bcc(vec![])))
        );
    }


    // 3.6.4.  Identification Fields
    #[test]
    fn test_message_id() {
        assert_eq!(
            header_field("Message-ID: <310@[127.0.0.1]>\r\n"),
            Ok(("", HeaderField::MessageID(model::MessageId { left: "310", right: "127.0.0.1" })))
        );
    }
    #[test]
    fn test_in_reply_to() {
        assert_eq!(
            header_field("In-Reply-To: <a@b> <c@example.com>\r\n"),
            Ok(("", HeaderField::InReplyTo(vec![
                model::MessageId { left: "a", right: "b" },
                model::MessageId { left: "c", right: "example.com" },
            ])))
        );
    }
    #[test]
    fn test_references() {
        assert_eq!(
            header_field("References: <1234@local.machine.example> <3456@example.net>\r\n"),
            Ok(("", HeaderField::References(vec![
                model::MessageId { left: "1234", right: "local.machine.example" },
                model::MessageId { left: "3456", right: "example.net" },
            ])))
        );
    }

    // 3.6.5.  Informational Fields
    #[test]
    fn test_subject() {
        assert_eq!(
            header_field("Subject: Aérogramme\r\n"),
            Ok(("", HeaderField::Subject("Aérogramme".into())))
        );
    }
    #[test]
    fn test_comments() {
        assert_eq!(
            header_field("Comments: 😛 easter egg!\r\n"),
            Ok(("", HeaderField::Comments("😛 easter egg!".into())))
        );
    }
    #[test]
    fn test_keywords() {
        assert_eq!(
            header_field("Keywords: fantasque, farfelu, fanfreluche\r\n"),
            Ok(("", HeaderField::Keywords(vec!["fantasque".into(), "farfelu".into(), "fanfreluche".into()])))
        );
    }

    // Test invalid field name
    #[test]
    fn test_invalid_field_name() {
        assert!(header_field("Unknown: unknown\r\n").is_err());
    }
}


