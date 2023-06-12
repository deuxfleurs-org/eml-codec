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

use crate::abnf::{fws, vchar_seq, perm_crlf};
use crate::model::{HeaderSection, HeaderDate};

/// HEADERS

/// Header section
///
/// See: https://www.rfc-editor.org/rfc/rfc5322.html#section-2.2
pub fn header_section(input: &str) -> IResult<&str, HeaderSection> {
    let (input, headers) = fold_many0(
        header_field,
        HeaderSection::default,
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

/// Extract one header field
///
/// Derived grammar inspired by RFC5322 optional-field:
/// 
/// ```abnf
/// field      =   field-name ":" unstructured CRLF
/// field-name =   1*ftext
/// ftext      =   %d33-57 /          ; Printable US-ASCII
///                %d59-126           ;  characters not including
///                                        ;  ":".
/// ```
fn header_field(input: &str) -> IResult<&str, HeaderField> {
    // Extract field name
    let (input, field_name) = take_while1(|c| c >= '\x21' && c <= '\x7E' && c != '\x3A')(input)?;
    let (input, _) = tuple((tag(":"), space0))(input)?;

    // Extract field body
    let (input, hfield) = match field_name {
        "Date" => {
            // @FIXME want to extract datetime our way in the future
            // to better handle obsolete/bad cases instead of crashing.
            let (input, raw_date) = unstructured(input)?;
            let date = match DateTime::parse_from_rfc2822(&raw_date) {
                Ok(chronodt) => HeaderDate::Parsed(chronodt),
                Err(e) => HeaderDate::Unknown(raw_date, e),
            };
            (input, HeaderField::Date(date))
        },
        //"From" => unimplemented!(),
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

    let (input, _) = crlf(input)?;
    return Ok((input, hfield));
}

/// Unstructured header field body
///
/// ```abnf
/// unstructured    =   (*([FWS] VCHAR_SEQ) *WSP) / obs-unstruct
/// ```
fn unstructured(input: &str) -> IResult<&str, String> {
    let (input, r) = many0(tuple((opt(fws), vchar_seq)))(input)?;
    let (input, _) = space0(input)?;

    // Try to optimize for the most common cases
    let body = match r.as_slice() {
        [(None, content)] => content.to_string(),
        [(Some(ws), content)] => ws.to_string() + content,
        lines => lines.iter().fold(String::with_capacity(255), |acc, item| {
            let (may_ws, content) = item;
            match may_ws {
                Some(ws) => acc + ws + content,
                None => acc + content,
            }
        }),
    };

    Ok((input, body))
}

