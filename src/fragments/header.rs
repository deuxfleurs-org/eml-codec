use std::borrow::Cow;
use nom::{
    IResult,
    branch::alt,
    bytes::complete::{is_not, take_while1, tag, tag_no_case},
    character::complete::space0,
    combinator::{map, opt, recognize},
    multi::{many0, many1, fold_many0, separated_list1},
    sequence::{terminated, preceded, pair, tuple},
};

use chardetng::EncodingDetector;
use encoding_rs::Encoding;

use crate::fragments::whitespace::{fws, perm_crlf};
use crate::fragments::misc_token::{phrase, unstructured};
use crate::fragments::model::{HeaderSection, Field, FieldBody};
use crate::fragments::mailbox::mailbox;
use crate::fragments::address::{mailbox_list, address_list, address_list_cfws};
use crate::fragments::identification::msg_id;
use crate::fragments::{datetime, trace};

/// HEADERS

///
pub fn from_bytes<'a>(rawmail: &'a [u8]) -> (Cow<'a, str>, &Encoding, bool) {
    // Create detector
    let mut detector = EncodingDetector::new();
    detector.feed(&rawmail, true);

    // Get encoding
    let enc: &Encoding = detector.guess(None, true);
    enc.decode(&rawmail)
}

/// Internal header section
///
/// See: https://www.rfc-editor.org/rfc/rfc5322.html#section-2.2
pub fn section<'a>(input: &'a str) -> IResult<&'a str, HeaderSection> {
    let (input, headers) = fold_many0(
        alt((known_field, unknown_field, rescue_field)),
        HeaderSection::default,
        |mut section, head| {
            match head {
                //@FIXME min and max limits are not enforced,
                // it may result in missing data or silently overriden data.

                // 3.6.1.  The Origination Date Field
                //   | orig-date      | 1      | 1          |                            |
                Field::Date(FieldBody::Correct(d)) => {
                    section.date = d;
                }

                // 3.6.2.  Originator Fields
                Field::From(FieldBody::Correct(v)) => {
                    //   | from           | 1      | 1          | See sender and 3.6.2       |
                    section.from = v;
                }
                Field::Sender(FieldBody::Correct(mbx)) => {
                    //   | sender         | 0*     | 1          | MUST occur with  multi-address from - see 3.6.2 |
                    section.sender = Some(mbx);
                }
                Field::ReplyTo(FieldBody::Correct(addr_list)) => {
                    //   | reply-to       | 0      | 1          |                            |
                    section.reply_to = addr_list;
                }

                // 3.6.3.  Destination Address Fields
                Field::To(FieldBody::Correct(addr_list)) => {
                    //    | to             | 0      | 1          |                            |
                    section.to = addr_list;
                }
                Field::Cc(FieldBody::Correct(addr_list)) => {
                    //    | cc             | 0      | 1          |                            |
                    section.cc = addr_list;
                }
                Field::Bcc(FieldBody::Correct(addr_list)) => {
                    //    | bcc            | 0      | 1          |                            |
                    section.bcc = addr_list;
                }

                // 3.6.4.  Identification Fields
                Field::MessageID(FieldBody::Correct(msg_id)) => {
                    //    | message-id     | 0*     | 1          | SHOULD be present - see  3.6.4 |
                    section.msg_id = Some(msg_id);
                }
                Field::InReplyTo(FieldBody::Correct(id_list)) => {
                    //    | in-reply-to    | 0*     | 1          | SHOULD occur in some replies - see 3.6.4  |
                    section.in_reply_to = id_list;
                }
                Field::References(FieldBody::Correct(id_list)) => {
                    //    | in-reply-to    | 0*     | 1          | SHOULD occur in some replies - see 3.6.4  |
                    section.references = id_list;
                }

                // 3.6.5.  Informational Fields
                Field::Subject(FieldBody::Correct(title)) => {
                    //    | subject        | 0      | 1          |                            |
                    section.subject = Some(title);
                }               
                Field::Comments(FieldBody::Correct(coms)) => {
                    //    | comments       | 0      | unlimited  |                            |
                    section.comments.push(coms);
                }
                Field::Keywords(FieldBody::Correct(mut kws)) => {
                    //    | keywords       | 0      | unlimited  |                            |
                    section.keywords.append(&mut kws);
                }

                // 3.6.6   Resent Fields are not implemented
                // 3.6.7   Trace Fields
                Field::ReturnPath(FieldBody::Correct(maybe_mbx)) => {
                    if let Some(mbx) = maybe_mbx {
                        section.return_path.push(mbx);
                    }
                }
                Field::Received(FieldBody::Correct(log)) => {
                    section.received.push(log);
                }

                // 3.6.8.  Optional Fields
                Field::Optional(name, body) => {
                    section.optional.insert(name, body);
                }

                // Rescue
                Field::Rescue(x) => {
                    section.unparsed.push(x);
                }

                bad_field => {
                   section.bad_fields.push(bad_field);
                }
            };
            section
        }
    )(input)?;
    
    let (input, _) = perm_crlf(input)?;
    Ok((input, headers))
}



/// Parse one known header field
///
/// RFC5322 optional-field seems to be a generalization of the field terminology.
/// We use it to parse all header names:
pub fn known_field(input: &str) -> IResult<&str, Field> {
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

/// A high-level function to match more easily a field name
fn field_name_tag(field_name: &str) -> impl FnMut(&str) -> IResult<&str, &str> + '_  {
    move |input: &str| {
        recognize(tuple((tag_no_case(field_name), space0, tag(":"), space0)))(input)
    }
}

// 3.6.1.  The Origination Date Field
fn date(input: &str) -> IResult<&str, Field> {
    map(preceded(field_name_tag("Date"), alt((
        map(datetime::section, |dt| FieldBody::Correct(dt)),
        map(rescue, |r| FieldBody::Failed(r))))),
    |b| Field::Date(b))(input)
}

// 3.6.2.  Originator Fields
fn from(input: &str) -> IResult<&str, Field> {
    map(preceded(field_name_tag("From"), alt((
        map(mailbox_list, |dt| FieldBody::Correct(dt)),
        map(rescue, |r| FieldBody::Failed(r))))),
    |b| Field::From(b))(input)
}
fn sender(input: &str) -> IResult<&str, Field> {
    map(preceded(field_name_tag("Sender"), alt((
        map(mailbox, |dt| FieldBody::Correct(dt)),
        map(rescue, |r| FieldBody::Failed(r))))),
    |b| Field::Sender(b))(input)
}
fn reply_to(input: &str) -> IResult<&str, Field> {
    map(preceded(field_name_tag("Reply-To"), alt((
        map(address_list, |dt| FieldBody::Correct(dt)),
        map(rescue, |r| FieldBody::Failed(r))))),
    |b| Field::ReplyTo(b))(input)
}

// 3.6.3.  Destination Address Fields
fn to(input: &str) -> IResult<&str, Field> {
    map(preceded(field_name_tag("To"), alt((
        map(address_list, |dt| FieldBody::Correct(dt)),
        map(rescue, |r| FieldBody::Failed(r))))),
    |b| Field::To(b))(input)
}
fn cc(input: &str) -> IResult<&str, Field> {
    map(preceded(field_name_tag("Cc"), alt((
        map(address_list, |dt| FieldBody::Correct(dt)),
        map(rescue, |r| FieldBody::Failed(r))))),
    |b| Field::Cc(b))(input)
}
fn bcc(input: &str) -> IResult<&str, Field> {
    map(preceded(field_name_tag("Bcc"), alt((
        map(opt(alt((address_list, address_list_cfws))), |dt| FieldBody::Correct(dt.unwrap_or(vec![]))),
        map(rescue, |r| FieldBody::Failed(r))))),
    |b| Field::Bcc(b))(input)
}

// 3.6.4.  Identification Fields
fn msg_id_field(input: &str) -> IResult<&str, Field> {
    map(preceded(field_name_tag("Message-ID"), alt((
        map(msg_id, |dt| FieldBody::Correct(dt)),
        map(rescue, |r| FieldBody::Failed(r))))),
    |b| Field::MessageID(b))(input)
}
fn in_reply_to(input: &str) -> IResult<&str, Field> {
    map(preceded(field_name_tag("In-Reply-To"), alt((
        map(many1(msg_id), |dt| FieldBody::Correct(dt)),
        map(rescue, |r| FieldBody::Failed(r))))),
    |b| Field::InReplyTo(b))(input)
}
fn references(input: &str) -> IResult<&str, Field> {
    map(preceded(field_name_tag("References"), alt((
        map(many1(msg_id), |dt| FieldBody::Correct(dt)),
        map(rescue, |r| FieldBody::Failed(r))))),
    |b| Field::References(b))(input)
}

// 3.6.5.  Informational Fields
fn subject(input: &str) -> IResult<&str, Field> {
    map(preceded(field_name_tag("Subject"), alt((
        map(unstructured, |dt| FieldBody::Correct(dt)),
        map(rescue, |r| FieldBody::Failed(r))))),
    |b| Field::Subject(b))(input)
}
fn comments(input: &str) -> IResult<&str, Field> {
    map(preceded(field_name_tag("Comments"), alt((
        map(unstructured, |dt| FieldBody::Correct(dt)),
        map(rescue, |r| FieldBody::Failed(r))))),
    |b| Field::Comments(b))(input)
}
fn keywords(input: &str) -> IResult<&str, Field> {
    map(preceded(field_name_tag("Keywords"), alt((
        map(separated_list1(tag(","), phrase), |dt| FieldBody::Correct(dt)),
        map(rescue, |r| FieldBody::Failed(r))))),
    |b| Field::Keywords(b))(input)
}


// 3.6.6 Resent fields
// Not implemented

// 3.6.7 Trace fields
fn return_path(input: &str) -> IResult<&str, Field> {
    map(preceded(field_name_tag("Return-Path"), alt((
        map(trace::return_path_body, |dt| FieldBody::Correct(dt)),
        map(rescue, |r| FieldBody::Failed(r))))),
    |b| Field::ReturnPath(b))(input)
}
fn received(input: &str) -> IResult<&str, Field> {
    map(preceded(field_name_tag("Received"), alt((
        map(trace::received_body, |dt| FieldBody::Correct(dt)),
        map(rescue, |r| FieldBody::Failed(r))))),
    |b| Field::Received(b))(input)
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
fn unknown_field(input: &str) -> IResult<&str, Field> {
    // Extract field name
    let (input, field_name) = field_name(input)?;
    let (input, body) = unstructured(input)?;
    let (input, _) = perm_crlf(input)?;
    Ok((input, Field::Optional(field_name, body)))
}
fn field_name(input: &str) -> IResult<&str, &str> {
    terminated(
        take_while1(|c| c >= '\x21' && c <= '\x7E' && c != '\x3A'),
        tuple((space0, tag(":"), space0))
    )(input)
}

/// Rescue rule
///
/// Something went wrong while parsing headers,
/// trying to fix parsing by consuming
/// one unfolded header line.
///
/// ```abnf
/// rescue = *(*any FWS) *any CRLF
fn rescue(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        many0(pair(is_not("\r\n"), fws)),
        is_not("\r\n"),
    ))(input)
}

fn rescue_field(input: &str) -> IResult<&str, Field> {
    map(terminated(rescue, perm_crlf), |r| Field::Rescue(r))(input)
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::fragments::model::{GroupRef, AddrSpec, MailboxRef, AddressRef};
    use crate::fragments::model;

    // 3.6.1.  The Origination Date Field
/*    #[test]
    fn test_datetime() {
        let datefield = "Date: Thu,\r\n  13\r\n  Feb\r\n    1969\r\n 23:32\r\n   -0330 (Newfoundland Time)\r\n";
        let (input, v) = known_field(datefield).unwrap();
        assert_eq!(input, "");
        match v {
            Field::Date(HeaderDate::Parsed(_)) => (),
            _ => panic!("Date has not been parsed"),
        };
    }*/

    // 3.6.2.  Originator Fields
    #[test]
    fn test_from() {
        assert_eq!(
            known_field("From: \"Joe Q. Public\" <john.q.public@example.com>\r\n"),
            Ok(("", Field::From(FieldBody::Correct(vec![MailboxRef { 
                name: Some("Joe Q. Public".into()), 
                addrspec: AddrSpec {
                    local_part: "john.q.public".into(),
                    domain: "example.com".into(),
                }
            }])))),
        );
    }
    #[test]
    fn test_sender() {
        assert_eq!(
            known_field("Sender: Michael Jones <mjones@machine.example>\r\n"),
            Ok(("", Field::Sender(FieldBody::Correct(MailboxRef {
                name: Some("Michael Jones".into()),
                addrspec: AddrSpec {
                    local_part: "mjones".into(),
                    domain: "machine.example".into(),
                },
            })))),
        );
    }
    #[test]
    fn test_reply_to() {
        assert_eq!(
            known_field("Reply-To: \"Mary Smith: Personal Account\" <smith@home.example>\r\n"),
            Ok(("", Field::ReplyTo(FieldBody::Correct(vec![AddressRef::Single(MailboxRef {
                name: Some("Mary Smith: Personal Account".into()),
                addrspec: AddrSpec {
                    local_part: "smith".into(),
                    domain: "home.example".into(),
                },
            })]))))
        );
    }

    // 3.6.3.  Destination Address Fields
    #[test]
    fn test_to() {
        assert_eq!(
            known_field("To: A Group:Ed Jones <c@a.test>,joe@where.test,John <jdoe@one.test>;\r\n"),
            Ok(("", Field::To(FieldBody::Correct(vec![AddressRef::Many(GroupRef {
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
            })]))))
        );
    }
    #[test]
    fn test_cc() {
        assert_eq!(
            known_field("Cc: Undisclosed recipients:;\r\n"),
            Ok(("", Field::Cc(FieldBody::Correct(vec![AddressRef::Many(GroupRef {
                name: "Undisclosed recipients".into(),
                participants: vec![],
            })]))))
        );
    }
    #[test]
    fn test_bcc() {
        assert_eq!(
            known_field("Bcc: (empty)\r\n"),
            Ok(("", Field::Bcc(FieldBody::Correct(vec![]))))
        );
        assert_eq!(
            known_field("Bcc: \r\n"),
            Ok(("", Field::Bcc(FieldBody::Correct(vec![]))))
        );
    }


    // 3.6.4.  Identification Fields
    #[test]
    fn test_message_id() {
        assert_eq!(
            known_field("Message-ID: <310@[127.0.0.1]>\r\n"),
            Ok(("", Field::MessageID(FieldBody::Correct(model::MessageId { left: "310", right: "127.0.0.1" }))))
        );
    }
    #[test]
    fn test_in_reply_to() {
        assert_eq!(
            known_field("In-Reply-To: <a@b> <c@example.com>\r\n"),
            Ok(("", Field::InReplyTo(FieldBody::Correct(vec![
                model::MessageId { left: "a", right: "b" },
                model::MessageId { left: "c", right: "example.com" },
            ]))))
        );
    }
    #[test]
    fn test_references() {
        assert_eq!(
            known_field("References: <1234@local.machine.example> <3456@example.net>\r\n"),
            Ok(("", Field::References(FieldBody::Correct(vec![
                model::MessageId { left: "1234", right: "local.machine.example" },
                model::MessageId { left: "3456", right: "example.net" },
            ]))))
        );
    }

    // 3.6.5.  Informational Fields
    #[test]
    fn test_subject() {
        assert_eq!(
            known_field("Subject: Aérogramme\r\n"),
            Ok(("", Field::Subject(FieldBody::Correct("Aérogramme".into()))))
        );
    }
    #[test]
    fn test_comments() {
        assert_eq!(
            known_field("Comments: 😛 easter egg!\r\n"),
            Ok(("", Field::Comments(FieldBody::Correct("😛 easter egg!".into()))))
        );
    }
    #[test]
    fn test_keywords() {
        assert_eq!(
            known_field("Keywords: fantasque, farfelu, fanfreluche\r\n"),
            Ok(("", Field::Keywords(FieldBody::Correct(vec!["fantasque".into(), "farfelu".into(), "fanfreluche".into()]))))
        );
    }

    // Test invalid field name
    #[test]
    fn test_invalid_field_name() {
        assert!(known_field("Unknown: unknown\r\n").is_err());
    }

    #[test]
    fn test_rescue_field() {
        assert_eq!(
            rescue_field("Héron: élan\r\n\tnoël: test\r\nFrom: ..."),
            Ok(("From: ...", Field::Rescue("Héron: élan\r\n\tnoël: test"))),
        );
    }

    #[test]
    fn test_wrong_fields() {
        let fullmail = r#"Return-Path: xoxo
From: !!!!

Hello world"#;
        assert_eq!(
            section(fullmail),
            Ok(("Hello world", HeaderSection {
                bad_fields: vec![
                    Field::ReturnPath(FieldBody::Failed("xoxo")),
                    Field::From(FieldBody::Failed("!!!!")),
                ],
                ..Default::default()
            }))
        );
    }
}
