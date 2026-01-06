use bounded_static::ToStatic;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_while},
    combinator::{map, opt},
    multi::many1,
    sequence::{delimited, pair, tuple},
    IResult,
};
use std::borrow::Cow;
use std::fmt;

use crate::display_bytes::{print_seq, Print, Formatter};
use crate::imf::mailbox::is_dtext;
use crate::text::whitespace::cfws;
use crate::text::words::dot_atom_text;

#[derive(PartialEq, Clone, ToStatic)]
pub struct MessageID<'a> {
    pub left: Cow<'a, [u8]>,
    pub right: Cow<'a, [u8]>,
}
impl<'a> ToString for MessageID<'a> {
    fn to_string(&self) -> String {
        format!(
            "{}@{}",
            String::from_utf8_lossy(&self.left),
            String::from_utf8_lossy(&self.right)
        )
    }
}
impl<'a> fmt::Debug for MessageID<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_tuple("MessageID")
            .field(&format_args!("\"{}\"", self.to_string()))
            .finish()
    }
}
// TODO: drop obs parts (when implemented?)
impl<'a> Print for MessageID<'a> {
    fn print(&self, fmt: &mut impl Formatter) -> std::io::Result<()> {
        fmt.write_bytes(b"<")?;
        fmt.write_bytes(&self.left)?;
        fmt.write_bytes(b"@")?;
        self.right.print(fmt)?;
        fmt.write_bytes(b">")
    }
}

// Must be non-empty
#[derive(PartialEq, Clone, Debug, ToStatic)]
pub struct MessageIDList<'a>(pub Vec<MessageID<'a>>);

impl<'a> Print for MessageIDList<'a> {
    fn print(&self, fmt: &mut impl Formatter) -> std::io::Result<()> {
        print_seq(fmt, &self.0, Formatter::write_fws)
    }
}

/// Message identifier
///
/// ```abnf
///    msg-id          =   [CFWS] "<" id-left "@" id-right ">" [CFWS]
/// ```
pub fn msg_id(input: &[u8]) -> IResult<&[u8], MessageID<'_>> {
    let (input, (left, _, right)) = delimited(
        pair(opt(cfws), tag("<")),
        tuple((id_left, tag("@"), id_right)),
        pair(tag(">"), opt(cfws)),
    )(input)?;
    let left = Cow::Borrowed(left);
    let right = Cow::Borrowed(right);
    Ok((input, MessageID { left, right }))
}

pub fn msg_list(input: &[u8]) -> IResult<&[u8], MessageIDList<'_>> {
    map(many1(msg_id), MessageIDList)(input)
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
                    left: Cow::Borrowed(&b"5678.21-Nov-1997"[..]),
                    right: Cow::Borrowed(&b"example.com"[..]),
                }
            )),
        );
    }
}
