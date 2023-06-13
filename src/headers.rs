use chrono::DateTime;
use nom::{
    IResult,
    bytes::complete::take_while1,
    bytes::complete::tag,
    character::complete::space0,
    combinator::opt,
    multi::fold_many0,
    multi::many0,
    sequence::tuple,
};

use crate::whitespace::{fws, perm_crlf};
use crate::words::vchar_seq;
use crate::misc_token::unstructured;
use crate::model::{PermissiveHeaderSection, HeaderDate, MailboxRef};
use crate::mailbox::mailbox;
use crate::address::{mailbox_list};

/// HEADERS

/// Header section
///
/// See: https://www.rfc-editor.org/rfc/rfc5322.html#section-2.2
pub fn header_section(input: &str) -> IResult<&str, PermissiveHeaderSection> {
    let (input, headers) = fold_many0(
        header_field,
        PermissiveHeaderSection::default,
        |mut section, head| {
            match head {
                // 3.6.1.  The Origination Date Field
                HeaderField::Date(d) => {
                    //@FIXME only one date is allowed, what are we doing if multiple dates are
                    //encountered? Currently, we override...
                    //   | orig-date      | 1      | 1          |                            |
                    section.date = d;
                }

                // 3.6.2.  Originator Fields
                HeaderField::From(v) => {
                    //@FIXME override the from field if declared multiple times.
                    //   | from           | 1      | 1          | See sender and 3.6.2       |
                    section.from = v;
                }
                HeaderField::Sender(mbx) => {
                    //   | sender         | 0*     | 1          | MUST occur with  multi-address from - see 3.6.2 |
                    section.sender = Some(mbx);
                }

                HeaderField::Subject(title) => {
                    section.subject = Some(title);
                }
                HeaderField::Optional(name, body) => {
                    section.optional.insert(name, body);
                }
                _ => unimplemented!(),
            };
            section
        }
    )(input)?;
    
    let (input, _) = perm_crlf(input)?;
    Ok((input, headers))
}

#[derive(Debug, PartialEq)]
enum HeaderField<'a> {
    // 3.6.1.  The Origination Date Field
    Date(HeaderDate),

    // 3.6.2.  Originator Fields
    From(Vec<MailboxRef>),
    Sender(MailboxRef),
    ReplyTo,

    // 3.6.3.  Destination Address Fields
    To,
    Cc,
    Bcc,

    // 3.6.4.  Identification Fields
    MessageID,
    InReplyTo,
    References,

    // 3.6.5.  Informational Fields
    Subject(String),
    Comments(String),
    Keywords(Vec<&'a str>),

    // 3.6.6.  Resent Fields
    ResentDate,
    ResentFrom,
    ResentSender,
    ResentTo,
    ResentCc,
    ResentBcc,
    ResentMessageID,

    // 3.6.7.  Trace Fields
    Trace,

    // 3.6.8.  Optional Fields
    Optional(&'a str, String)
}

/// Parse one header field
///
/// RFC5322 optional-field seems to be a generalization of the field terminology.
/// We use it to parse all header names:
/// 
/// ```abnf
/// field      =   field-name ":" unstructured CRLF
/// field-name =   1*ftext
/// ftext      =   %d33-57 /          ; Printable US-ASCII
///                %d59-126           ;  characters not including
///                                   ;  ":".
/// ```
fn header_field(input: &str) -> IResult<&str, HeaderField> {
    // Extract field name
    let (input, field_name) = take_while1(|c| c >= '\x21' && c <= '\x7E' && c != '\x3A')(input)?;
    let (input, _) = tuple((tag(":"), space0))(input)?;

    // Extract field body
    let (input, hfield) = match field_name {
        "Date" => datetime(input)?,
        "From" => {
            let (input, body) = mailbox_list(input)?;
            (input, HeaderField::From(body))
        },
        "Sender" => {
            let (input, body) = mailbox(input)?;
            (input, HeaderField::Sender(body))
        },
        "Subject" => {
            let (input, body) = unstructured(input)?;
            (input, HeaderField::Subject(body))
        },
        _ => {
            let (input, body) = unstructured(input)?;
            (input, HeaderField::Optional(field_name, body))
        }
    };

    // Drop EOL
    let (input, _) = perm_crlf(input)?;
    return Ok((input, hfield));
}

fn datetime(input: &str) -> IResult<&str, HeaderField> {
    // @FIXME want to extract datetime our way in the future
    // to better handle obsolete/bad cases instead of returning raw text.
    let (input, raw_date) = unstructured(input)?;
    let date = match DateTime::parse_from_rfc2822(&raw_date) {
        Ok(chronodt) => HeaderDate::Parsed(chronodt),
        Err(e) => HeaderDate::Unknown(raw_date, e),
    };
    Ok((input, HeaderField::Date(date)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AddrSpec;

    #[test]
    fn test_datetime() {
        let datefield = "Thu,\r\n  13\r\n  Feb\r\n    1969\r\n 23:32\r\n   -0330 (Newfoundland Time)";
        let (input, v) = datetime(datefield).unwrap();
        assert_eq!(input, "");
        match v {
            HeaderField::Date(HeaderDate::Parsed(_)) => (),
            _ => panic!("Date has not been parsed"),
        };
    }

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
}


