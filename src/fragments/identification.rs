use std::borrow::Cow;
use nom::{
    IResult,
    branch::alt,
    bytes::complete::{take_while, tag},
    combinator::opt,
    multi::many1,
    sequence::{delimited, pair, tuple},
};

use crate::fragments::lazy;
use crate::fragments::whitespace::cfws;
use crate::fragments::words::dot_atom_text;
use crate::fragments::mailbox::is_dtext;
use crate::fragments::model::{MessageId, MessageIdList};
use crate::error::IMFError;

impl<'a> TryFrom<lazy::Identifier<'a>> for MessageId<'a> {
    type Error = IMFError<'a>;

    fn try_from(id: lazy::Identifier<'a>) -> Result<Self, Self::Error> {
        msg_id(id.0)
            .map(|(_, i)| i)
            .map_err(|e| IMFError::MessageID(e))
    }
}

impl<'a> TryFrom<lazy::IdentifierList<'a>> for MessageIdList<'a> {
    type Error = IMFError<'a>;

    fn try_from(id: lazy::IdentifierList<'a>) -> Result<Self, Self::Error> {
        many1(msg_id)(id.0)
            .map(|(_, i)| i)
            .map_err(|e| IMFError::MessageIDList(e))
    }
}

/// Message identifier
///
/// ```abnf
///    msg-id          =   [CFWS] "<" id-left "@" id-right ">" [CFWS]
/// ```
pub fn msg_id(input: &str) -> IResult<&str, MessageId> {
    let (input, (left, _, right)) = delimited(
        pair(opt(cfws), tag("<")),
        tuple((id_left, tag("@"), id_right)),
        pair(tag(">"), opt(cfws)),
    )(input)?;
    Ok((input, MessageId{ left, right }))
}

// Missing obsolete
fn id_left(input: &str) -> IResult<&str, &str> {
    dot_atom_text(input)   
}

// Missing obsolete
fn id_right(input: &str) -> IResult<&str, &str> {
    alt((dot_atom_text, no_fold_litteral))(input)
}

fn no_fold_litteral(input: &str) -> IResult<&str, &str> {
    delimited(tag("["), take_while(is_dtext), tag("]"))(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_msg_id() {
        assert_eq!(
            msg_id("<5678.21-Nov-1997@example.com>"),
            Ok(("", MessageId{left: "5678.21-Nov-1997", right: "example.com"})),
        );
    }
}
