use nom::{
    IResult,
    
}
use crate::model;

enum RestField<'a> {
    // 3.6.6.  Resent Fields
    ResentDate(HeaderDate),
    ResentFrom(Vec<MailboxRef>),
    ResentSender(MailboxRef),
    ResentTo(Vec<AddressRef>),
    ResentCc(Vec<AddressRef>),
    ResentBcc(Vec<AddressRef>),
    ResentMessageID(model::MessageId<'a>),

    // 3.6.8. Optional fields
    Optional(&'a str, String),
}

enum PreludeField<'a> {
    // 3.6.7.  Trace Fields
    ReturnPath(String),
    Received(Vec<String>),
}

/// Section
///
/// Rewritten section for more compatibility
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
    let (input, mut prelude_trace) = prelude(input)?;
    let (input, full_trace) = fold_many0(
        rest_field,
        prelude_trace,
        |mut trace, field| {
            match field {
                
            }
        }

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
fn prelude(input: &str) -> IResult<&str, model::Trace> {
}

fn rest_field(input: &str) -> IResult<&str, RestField> {
    // Ensure this is not a new prelude
}
