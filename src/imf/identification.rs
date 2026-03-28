#[cfg(feature = "arbitrary")]
use arbitrary::Arbitrary;
use bounded_static::ToStatic;
use nom::{
    branch::alt,
    bytes::complete::tag,
    combinator::{map, opt},
    multi::many1,
    sequence::{delimited, pair, terminated, tuple},
    IResult,
};

#[cfg(feature = "arbitrary")]
use crate::fuzz_eq::FuzzEq;
#[cfg(feature = "tracing")]
use crate::utils::bytes_to_display_string;
use crate::i18n::ContainsUtf8;
use crate::print::{print_seq, Print, Formatter, ToStringFromPrint};
use crate::imf::mailbox::{dtext, Dtext};
use crate::text::whitespace::cfws;
use crate::text::words::{dot_atom_text, DotAtom};

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
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = bytes_to_display_string(input)))
)]
pub fn msg_id(input: &[u8]) -> IResult<&[u8], MessageID<'_>> {
    let (input, (left, _, right)) = delimited(
        pair(opt(cfws), tag("<")),
        tuple((id_left, tag("@"), id_right)),
        pair(tag(">"), opt(cfws)),
    )(input)?;
    Ok((input, MessageID { left, right }))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = bytes_to_display_string(input)))
)]
pub fn msg_list(input: &[u8]) -> IResult<&[u8], MessageIDList<'_>> {
    // The "," separator is not specified in the RFC but some real-world emails
    // use it.
    // TODO: obs-references and obs-in-reply-to
    many1(terminated(msg_id, opt(tag(","))))(input)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = bytes_to_display_string(input)))
)]
pub fn nullable_msg_list(input: &[u8]) -> IResult<&[u8], MessageIDList<'_>> {
    alt((
        msg_list,
        map(opt(cfws), |_| vec![]),
    ))(input)
}

// @FIXME Missing obsolete
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = bytes_to_display_string(input)))
)]
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
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = bytes_to_display_string(input)))
)]
fn id_right(input: &[u8]) -> IResult<&[u8], MessageIDRight<'_>> {
    alt((
        map(dot_atom_text, MessageIDRight::DotAtom),
        map(no_fold_literal, MessageIDRight::Literal)
    ))(input)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = bytes_to_display_string(input)))
)]
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

    // TODO: non-compliant but found in aero100
    // <523C50DA-160C-4550-A44E-7E192513CF91>

    #[test]
    fn test_comma_separated_msg_list() {
        // This is not RFC-valid syntax but was encountered in real-world emails
        assert_eq!(
            msg_list(b" <8d9bb189354d4804bcc2fd1d1a5398b5@cnrs.fr>,<ef8fac8b36834864bae895571064565c@cnrs.fr>"),
            Ok((
                &b""[..],
                vec![
                    MessageID {
                        left: DotAtom("8d9bb189354d4804bcc2fd1d1a5398b5"[..].into()),
                        right: MessageIDRight::DotAtom(DotAtom("cnrs.fr"[..].into())),
                    },
                    MessageID {
                        left: DotAtom("ef8fac8b36834864bae895571064565c"[..].into()),
                        right: MessageIDRight::DotAtom(DotAtom("cnrs.fr"[..].into())),
                    },
                ]
            ))
        );
    }
}
