use nom::{
    branch::alt,
    bytes::complete::{tag, take_while},
    combinator::opt,
    sequence::{delimited, pair, tuple},
    IResult,
};

use crate::rfc5322::mailbox::is_dtext;
use crate::text::whitespace::cfws;
use crate::text::words::dot_atom_text;


#[derive(Debug, PartialEq)]
pub struct MessageId<'a> {
    pub left: &'a [u8],
    pub right: &'a [u8],
}
pub type MessageIdList<'a> = Vec<MessageId<'a>>;

/*
impl<'a> TryFrom<&'a lazy::Identifier<'a>> for MessageId<'a> {
    type Error = IMFError<'a>;

    fn try_from(id: &'a lazy::Identifier<'a>) -> Result<Self, Self::Error> {
        msg_id(id.0)
            .map(|(_, i)| i)
            .map_err(|e| IMFError::MessageID(e))
    }
}

impl<'a> TryFrom<&'a lazy::IdentifierList<'a>> for MessageIdList<'a> {
    type Error = IMFError<'a>;

    fn try_from(id: &'a lazy::IdentifierList<'a>) -> Result<Self, Self::Error> {
        many1(msg_id)(id.0)
            .map(|(_, i)| i)
            .map_err(|e| IMFError::MessageIDList(e))
    }
}*/

/// Message identifier
///
/// ```abnf
///    msg-id          =   [CFWS] "<" id-left "@" id-right ">" [CFWS]
/// ```
pub fn msg_id(input: &[u8]) -> IResult<&[u8], MessageId> {
    let (input, (left, _, right)) = delimited(
        pair(opt(cfws), tag("<")),
        tuple((id_left, tag("@"), id_right)),
        pair(tag(">"), opt(cfws)),
    )(input)?;
    Ok((input, MessageId { left, right }))
}

// @FIXME Missing obsolete
fn id_left(input: &[u8]) -> IResult<&[u8], &[u8]> {
    dot_atom_text(input)
}

// @FIXME Missing obsolete
fn id_right(input: &[u8]) -> IResult<&[u8], &[u8]> {
    alt((dot_atom_text, no_fold_litteral))(input)
}

fn no_fold_litteral(input: &[u8]) -> IResult<&[u8], &[u8]> {
    delimited(tag("["), take_while(is_dtext), tag("]"))(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_msg_id() {
        assert_eq!(
            msg_id(b"<5678.21-Nov-1997@example.com>"),
            Ok((
                &b""[..],
                MessageId {
                    left: &b"5678.21-Nov-1997"[..],
                    right: &b"example.com"[..],
                }
            )),
        );
    }
}
