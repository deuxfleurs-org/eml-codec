#[cfg(feature = "arbitrary")]
use arbitrary::Arbitrary;
#[cfg(feature = "tracing-recover")]
use tracing::warn;
use bounded_static::ToStatic;
use nom::{
    branch::alt,
    bytes::complete::tag,
    combinator::{map, opt},
    multi::many0,
    sequence::{delimited, pair, tuple},
    IResult,
};

#[cfg(feature = "arbitrary")]
use crate::fuzz_eq::FuzzEq;
#[cfg(feature = "tracing")]
use crate::utils::bytes_to_trace_string;
use crate::i18n::ContainsUtf8;
use crate::print::{print_seq, Print, Formatter, ToStringFromPrint};
use crate::imf::mailbox::{domain, dtext, local_part, Domain, Dtext, LocalPart};
use crate::text::whitespace::cfws;
use crate::text::words::{dot_atom, dot_atom_text, DotAtom};

// NOTE: MessageID is not strictly RFC-compliant, printing it may use obsolete
// or non-compliant syntax.
#[derive(Clone, ContainsUtf8, Debug, PartialEq, ToStatic, ToStringFromPrint)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary, FuzzEq))]
pub enum MessageID<'a> {
    ObsLeftRight { left: LocalPart<'a>, right: Domain<'a> },
    InvalidAtom(DotAtom<'a>),
}
impl<'a> Print for MessageID<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        fmt.write_bytes(b"<");
        match &self {
            MessageID::ObsLeftRight { left, right } => {
                left.print(fmt);
                fmt.write_bytes(b"@");
                right.print(fmt);
            },
            MessageID::InvalidAtom(a) =>
                a.print(fmt),
        }
        fmt.write_bytes(b">");
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
/// The RFC gives the following syntax:
/// ```abnf
///    msg-id          =   [CFWS] "<" id-left "@" id-right ">" [CFWS]
/// ```
///
/// but we also handle invalid syntax found in the real-world:
/// ```abnf
///    our-msg-id      = msg-id / "<" dot-atom-text ">" / dot-atom
/// ```
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = %bytes_to_trace_string(input)))
)]
pub fn msg_id(input: &[u8]) -> IResult<&[u8], MessageID<'_>> {
    alt((
        msg_id_angle,
        map(dot_atom, |a| {
            #[cfg(feature = "tracing-recover")]
            warn!("message-id: bare atom");
            MessageID::InvalidAtom(a)
        }),
    ))(input)
}
pub fn msg_id_angle(input: &[u8]) -> IResult<&[u8], MessageID<'_>> {
    delimited(
        pair(opt(cfws), tag("<")),
        alt((
            map(tuple((id_left, tag("@"), id_right)), |(left, _, right)| {
                MessageID::ObsLeftRight { left, right }
            }),
            map(dot_atom_text, |a| {
                #[cfg(feature = "tracing-recover")]
                warn!("message-id: bare <atom>");
                MessageID::InvalidAtom(a)
            }),
        )),
        pair(tag(">"), opt(cfws)),
    )(input)
}

/// A *very* lenient parser for lists of msg_id as used by In-Reply-To and References
///
/// The RFC definition is:
/// ```abnf
///       in-reply-to    =    1*msg-id
///   obs-in-reply-to    =    *(phrase / msg-id)
/// ```
/// In the obs- syntax, the phrase tokens must be ignored.
///
/// However, historical emails seem to contain a lot of nonsense in between msg-id,
/// a lot of it not part of the "phrase" syntax. We implement a more lenient parser
/// that skips quoted strings, encoded words (both part of the phrase syntax), and
/// as a last resort, any bytes until encountering something that could be the start
/// of one of the more "structured" tokens (msg-id, encoded word, quoted string).
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = %bytes_to_trace_string(input)))
)]
pub fn nullable_msg_list(input: &[u8]) -> IResult<&[u8], MessageIDList<'_>> {
    use crate::text::quoted::quoted_string;
    use crate::text::encoding::{encoded_word, Context};
    let (input, tokens) = many0(alt((
        map(msg_id_angle, Some),
        map(cfws, |_| None),
        map(quoted_string, |_| None),
        map(encoded_word(Context::Phrase), |_| None),
        // skip until encountering either a < (for the beginning of msg-id), a =
        // (for the beginning of encoded-word), or a \" (beginning of quoted
        // string).
        map(nom::bytes::complete::is_not("<=\""), |_| None),
    )))(input)?;

    Ok((input, tokens.into_iter().flatten().collect()))
}

/// Implements obs-id-left, which is a superset of id-left:
/// ```abnf
///     id-left     =   dot-atom-text / obs-id-left
/// obs-id-left     =   local-part
/// ```
///
/// NOTE: this directly returns the AST corresponding to *possibly obsolete*
/// syntax; we do not attempt to "strictify" it
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = %bytes_to_trace_string(input)))
)]
fn id_left(input: &[u8]) -> IResult<&[u8], LocalPart<'_>> {
    local_part(input)
}

/// Implements obs-id-right, which is a superset of id-right:
/// ```abnf
///     id-right     =   dot-atom-text / no-fold-literal / obs-id-right
/// obs-id-right     =   domain
/// ```
///
/// NOTE: this directly returns the AST corresponding to *possibly obsolete*
/// syntax; we do not attempt to "strictify" it
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = %bytes_to_trace_string(input)))
)]
fn id_right(input: &[u8]) -> IResult<&[u8], Domain<'_>> {
    domain(input)
}

#[allow(dead_code)]
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = %bytes_to_trace_string(input)))
)]
fn no_fold_literal(input: &[u8]) -> IResult<&[u8], Dtext<'_>> {
    delimited(tag("["), dtext, tag("]"))(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::misc_token::Word;
    use crate::text::quoted::QuotedString;
    use crate::text::words::Atom;
    use crate::imf::mailbox::{Domain, LocalPart, LocalPartToken};
    use crate::print::tests::print_to_vec;

    fn assert_msg_list_reprinted(txt: &[u8], printed: &[u8]) {
        let (rest, parsed) = nullable_msg_list(txt).unwrap();
        assert_eq!(rest, b"");
        let reprinted = print_to_vec(parsed);
        assert_eq!(String::from_utf8_lossy(&reprinted), String::from_utf8_lossy(printed));
    }

    #[test]
    fn test_msg_id() {
        assert_eq!(
            msg_id(b"<5678.21-Nov-1997@example.com>"),
            Ok((
                &b""[..],
                MessageID::ObsLeftRight {
                    left: LocalPart(vec![
                        LocalPartToken::Word(Word::Atom(Atom("5678".into()))),
                        LocalPartToken::Dot,
                        LocalPartToken::Word(Word::Atom(Atom("21-Nov-1997".into()))),
                    ]),
                    right: Domain::Atoms(vec![
                        Atom("example".into()),
                        Atom("com".into()),
                    ]),
                }
            )),
        );
    }

    #[test]
    fn test_obsolete_msg_id() {
        assert_eq!(
            msg_id(b" < foo . bar@univ-valenciennes  .fr >"),
            Ok((
                &b""[..],
                MessageID::ObsLeftRight {
                    left: LocalPart(vec![
                        LocalPartToken::Word(Word::Atom(Atom("foo".into()))),
                        LocalPartToken::Dot,
                        LocalPartToken::Word(Word::Atom(Atom("bar".into()))),
                    ]),
                    right: Domain::Atoms(vec![
                        Atom("univ-valenciennes".into()),
                        Atom("fr".into()),
                    ]),
                }
            )),
        );

        assert_eq!(
            msg_id(b"<\"24806 Tue Sep 19 11:05:34 1995\"@bnr.ca>"),
            Ok((
                &b""[..],
                MessageID::ObsLeftRight {
                    left: LocalPart(vec![
                        LocalPartToken::Word(Word::Quoted(
                            QuotedString(vec![
                                "24806".into(), " ".into(),
                                "Tue".into(), " ".into(),
                                "Sep".into(), " ".into(),
                                "19".into(), " ".into(),
                                "11:05:34".into(), " ".into(),
                                "1995".into(),
                            ])
                        ))
                    ]),
                    right: Domain::Atoms(vec![
                        Atom("bnr".into()),
                        Atom("ca".into()),
                    ]),
                }
            )),
        );
    }

    #[test]
    fn test_noncompliant_msg_id() {
        assert_eq!(
            msg_id(b" <523C50DA-160C-4550-A44E-7E192513CF91> "),
            Ok((&b""[..], MessageID::InvalidAtom(DotAtom("523C50DA-160C-4550-A44E-7E192513CF91".into()))))
        );

        assert_eq!(
            msg_id(b" foo "),
            Ok((&b""[..], MessageID::InvalidAtom(DotAtom("foo".into()))))
        );
    }

    #[test]
    fn test_comma_separated_msg_list() {
        // This is not RFC-valid syntax but was encountered in real-world emails
        assert_eq!(
            nullable_msg_list(b" <8d9bb189354d4804bcc2fd1d1a5398b5@cnrs.fr>,<ef8fac8b36834864bae895571064565c@cnrs.fr>"),
            Ok((
                &b""[..],
                vec![
                    MessageID::ObsLeftRight {
                        left: LocalPart(vec![
                            LocalPartToken::Word(Word::Atom(Atom("8d9bb189354d4804bcc2fd1d1a5398b5".into()))),
                        ]),
                        right: Domain::Atoms(vec![
                            Atom("cnrs".into()),
                            Atom("fr".into()),
                        ]),
                    },
                    MessageID::ObsLeftRight {
                        left: LocalPart(vec![
                            LocalPartToken::Word(Word::Atom(Atom("ef8fac8b36834864bae895571064565c".into()))),
                        ]),
                        right: Domain::Atoms(vec![
                            Atom("cnrs".into()),
                            Atom("fr".into()),
                        ]),
                    },
                ]
            ))
        );
    }

    #[test]
    fn test_msg_list_weird() {
        assert_msg_list_reprinted(
            b"<3AB624F9.5B6C6680@example.com>; from foo@example.com on Mon, Mar 19, 2001 at 04:25:45PM +0100",
            b"<3AB624F9.5B6C6680@example.com>"
        );

        assert_msg_list_reprinted(
            b"<3AB624F9.5B6C6680@example.com> from \"Foo bar\" on Mon, Mar 19, 2001 at 04:25:45 AM",
            b"<3AB624F9.5B6C6680@example.com>"
        );
    }
}
