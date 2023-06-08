use std::collections::HashMap;
use std::default::Default;

use nom::{
    IResult, 
    branch::alt,
    combinator::opt,
    character::complete::alphanumeric1,
    character::complete::crlf,
    character::complete::space0,
    character::complete::space1,
    bytes::complete::tag,
    bytes::complete::take_while1,
    multi::many0,
    multi::fold_many0,
    sequence::terminated,
    sequence::tuple,
};

//-------------- ABNF rfc5234

/// Permissive CRLF
///
/// Theoretically, all lines must end with \r\n
/// but mail servers support malformated emails,
/// for example with only \n eol. It works because
/// \r\n is allowed nowhere else, so we also add this support.
fn perm_crlf(input: &str) -> IResult<&str, &str> {
    alt((crlf, tag("\r"), tag("\n")))(input)
}

// Note: WSP = SP / HTAB = %x20 / %x09
// nom::*::space0 = *WSP
// nom::*::space1 = 1*WSP

/// Parse a folding white space
///
/// Folding white space are used for long headers splitted on multiple lines
///
/// ```abnf
/// FWS             =   ([*WSP CRLF] 1*WSP) /  obs-FWS
/// obs-FWS         =   1*WSP *(CRLF 1*WSP)
/// ```
fn fws(input: &str) -> IResult<&str, &str> {
    let (input, _) = opt(terminated(space0, perm_crlf))(input)?;
    // @FIXME: not implemented obs-FWS
    space1(input)
}

/// Sequence of visible chars with the UTF-8 extension
///
/// ```abnf
/// VCHAR   =  %x21-7E
///            ; visible (printing) characters
/// VCHAR   =/  UTF8-non-ascii
/// SEQ     = 1*VCHAR
///```
fn vchar_seq(input: &str) -> IResult<&str, &str> {
   take_while1(|c: char| (c >= '\x21' && c <= '\x7E') || !c.is_ascii())(input)
}

//--------------- HEADERS
/// Header section
///
/// See: https://www.rfc-editor.org/rfc/rfc5322.html#section-2.2
#[derive(Debug, Default)]
pub struct HeaderSection<'a> {
    pub subject: Option<String>,
    pub from: Vec<String>,
    pub optional: HashMap<&'a str, String>,
}

fn header_section(input: &str) -> IResult<&str, HeaderSection> {
    fold_many0(
        header_field,
        HeaderSection::default,
        |mut section, head| {
            match head {
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
    )(input)
}

enum HeaderField<'a> {
    // 3.6.1.  The Origination Date Field
    Date,

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
    match field_name {
        "Date" => unimplemented!(),
        //"From" => unimplemented!(),
        "Sender" => unimplemented!(),
        "Subject" => {
            let (input, body) = unstructured(input)?;
            let (input, _) = crlf(input)?;
            Ok((input, HeaderField::Subject(body)))
        },
        _ => {
            let (input, body) = unstructured(input)?;
            let (input, _) = crlf(input)?;
            Ok((input, HeaderField::Optional(field_name, body)))
        }
    }
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
        lines => lines.iter().fold(String::with_capacity(255), |mut acc, item| {
            let (may_ws, content) = item;
            match may_ws {
                Some(ws) => acc + ws + content,
                None => acc + content,
            }
        }),
    };

    Ok((input, body))
}

fn main() {
    let header = "Subject: Hello\r\n World\r\nFrom: <quentin@deuxfleurs.fr>\r\n\r\n";
    println!("{:?}", header_section(header));
}
