#[cfg(feature = "arbitrary")]
use arbitrary::Arbitrary;
use bounded_static::ToStatic;
use nom::{
    branch::alt,
    bytes::complete::tag,
    combinator::{map, opt},
    multi::many1,
    sequence::{delimited, pair, tuple},
    IResult,
};

#[cfg(feature = "arbitrary")]
use crate::fuzz_eq::FuzzEq;
use crate::print::{print_seq, Print, Formatter, ToStringFromPrint};
use crate::imf::mailbox::{dtext, Dtext};
use crate::text::whitespace::cfws;
use crate::text::words::{dot_atom_text, DotAtom};
use crate::utils::ContainsUtf8;

#[derive(Clone, ContainsUtf8, Debug, PartialEq, ToStatic, ToStringFromPrint)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary, FuzzEq))]
pub struct MessageID<'a> {
    pub left: DotAtom<'a>,
    pub right: MessageIDRight<'a>,
}
// TODO: drop obs parts (when implemented?)
impl<'a> Print for MessageID<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        fmt.write_bytes(b"<");
        self.left.print(fmt);
        fmt.write_bytes(b"@");
        self.right.print(fmt);
        fmt.write_bytes(b">")
    }
}

// Must be non-empty
pub type MessageIDList<'a> = Vec<MessageID<'a>>;

impl<'a> Print for MessageIDList<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        print_seq(fmt, &self, Formatter::write_fws)
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
    Ok((input, MessageID { left, right }))
}

pub fn msg_list(input: &[u8]) -> IResult<&[u8], MessageIDList<'_>> {
    many1(msg_id)(input)
}

// @FIXME Missing obsolete
fn id_left(input: &[u8]) -> IResult<&[u8], DotAtom<'_>> {
    dot_atom_text(input)
}

#[derive(Clone, ContainsUtf8, Debug, PartialEq, ToStatic, ToStringFromPrint)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary, FuzzEq))]
pub enum MessageIDRight<'a> {
    DotAtom(DotAtom<'a>),
    Literal(Dtext<'a>),
}
impl<'a> Print for MessageIDRight<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        match self {
            MessageIDRight::DotAtom(a) => a.print(fmt),
            MessageIDRight::Literal(dt) => {
                fmt.write_bytes(b"[");
                dt.print(fmt);
                fmt.write_bytes(b"]");
            },
        }
    }
}

// @FIXME Missing obsolete
fn id_right(input: &[u8]) -> IResult<&[u8], MessageIDRight<'_>> {
    alt((
        map(dot_atom_text, MessageIDRight::DotAtom),
        map(no_fold_literal, MessageIDRight::Literal)
    ))(input)
}

fn no_fold_literal(input: &[u8]) -> IResult<&[u8], Dtext<'_>> {
    delimited(tag("["), dtext, tag("]"))(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::words::DotAtom;

    #[test]
    fn test_msg_id() {
        assert_eq!(
            msg_id(b"<5678.21-Nov-1997@example.com>"),
            Ok((
                &b""[..],
                MessageID {
                    left: DotAtom("5678.21-Nov-1997"[..].into()),
                    right: MessageIDRight::DotAtom(DotAtom("example.com"[..].into())),
                }
            )),
        );
    }
}
