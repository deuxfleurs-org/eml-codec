use nom::{
    branch::alt,
    bytes::complete::{tag, take_while},
    combinator::opt,
    multi::many1,
    sequence::{delimited, pair, tuple},
    IResult,
};

use crate::rfc5322::mailbox::is_dtext;
use crate::text::whitespace::cfws;
use crate::text::words::dot_atom_text;


#[derive(Debug, PartialEq)]
pub struct MessageID<'a> {
    pub left: &'a [u8],
    pub right: &'a [u8],
}
pub type MessageIDList<'a> = Vec<MessageID<'a>>;

/// Message identifier
///
/// ```abnf
///    msg-id          =   [CFWS] "<" id-left "@" id-right ">" [CFWS]
/// ```
pub fn msg_id(input: &[u8]) -> IResult<&[u8], MessageID> {
    let (input, (left, _, right)) = delimited(
        pair(opt(cfws), tag("<")),
        tuple((id_left, tag("@"), id_right)),
        pair(tag(">"), opt(cfws)),
    )(input)?;
    Ok((input, MessageID { left, right }))
}

pub fn msg_list(input: &[u8]) -> IResult<&[u8], MessageIDList> {
    many1(msg_id)(input)
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
                MessageID {
                    left: &b"5678.21-Nov-1997"[..],
                    right: &b"example.com"[..],
                }
            )),
        );
    }
}
