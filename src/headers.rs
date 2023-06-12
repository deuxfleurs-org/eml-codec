use chrono::DateTime;
use nom::{
    IResult,
    bytes::complete::take_while1,
    bytes::complete::tag,
    character::complete::space0,
    character::complete::crlf,
    combinator::opt,
    multi::fold_many0,
    multi::many0,
    sequence::tuple,
};

use crate::tokens::{fws, vchar_seq, perm_crlf, unstructured};
use crate::model::{PermissiveHeaderSection, HeaderDate, MailboxRef};

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
                HeaderField::Date(d) => {
                    //@FIXME only one date is allowed, what are we doing if multiple dates are
                    //encountered? Currently, we override...
                    section.date = d;
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

#[derive(Debug)]
enum HeaderField<'a> {
    // 3.6.1.  The Origination Date Field
    Date(HeaderDate),

    // 3.6.2.  Originator Fields
    From,
    Sender,
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
        "From" => from(input)?,
        "Sender" => unimplemented!(),
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
    let (input, _) = crlf(input)?;
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

fn from(input: &str) -> IResult<&str, HeaderField> {
    //let (input, mbox_list) = many0(mailbox)(input)?;
    //Ok((input, HeaderField::From(mbox_list)))
    unimplemented!();
}

fn mailbox(input: &str) -> IResult<&str, MailboxRef> {
    unimplemented!();
}

#[cfg(test)]
mod tests {
    use super::*;

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
}


