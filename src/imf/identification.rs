#[cfg(feature = "arbitrary")]
use arbitrary::Arbitrary;
use bounded_static::ToStatic;
use nom::{
    branch::alt,
    bytes::complete::tag,
    combinator::{eof, map, opt, recognize},
    multi::many0,
    sequence::{delimited, pair, preceded, tuple},
    IResult,
};
use std::borrow::Cow;
#[cfg(any(feature = "tracing-recover", feature = "tracing-unsupported"))]
use tracing::warn;

use crate::i18n::ContainsUtf8;
use crate::imf::mailbox::{domain, dtext, local_part, Domain, Dtext, LocalPart};
use crate::print::{print_seq, Formatter, Print, ToStringFromPrint};
use crate::text::recovery::{take_quoted_encoded_or_until1, take_quoted_or_until};
use crate::text::utf8::{is_nonascii_or, take_utf8_while1};
use crate::text::whitespace::cfws;
#[cfg(any(feature = "tracing-recover", feature = "tracing-unsupported"))]
use crate::utils::bytes_to_trace_string;
#[cfg(feature = "arbitrary")]
use crate::{arbitrary_utils::arbitrary_string_nonempty_where, fuzz_eq::FuzzEq};
use eml_codec_derives::instrument_input;

// NOTE: MessageID is not strictly RFC-compliant, printing it may use obsolete
// or non-compliant syntax.
#[derive(Clone, ContainsUtf8, Debug, PartialEq, ToStatic, ToStringFromPrint)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub enum MessageID<'a> {
    // The compliant (but possibly obsolete) syntax
    ObsLeftRight {
        left: LocalPart<'a>,
        right: Domain<'a>,
    },
    // Non-compliant char sequence (must be non-empty and satisfy is_invalid_msgid_text)
    #[cfg_attr(feature = "arbitrary", fuzz_eq(use_eq))]
    Invalid(Cow<'a, str>),
}
impl<'a> Print for MessageID<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        fmt.write_bytes(b"<");
        match &self {
            MessageID::ObsLeftRight { left, right } => {
                left.print(fmt);
                fmt.write_bytes(b"@");
                right.print(fmt);
            }
            MessageID::Invalid(txt) => fmt.write_bytes(txt.as_bytes()),
        }
        fmt.write_bytes(b">");
    }
}
#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for MessageID<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        match u.int_in_range(0..=1)? {
            0 => Ok(MessageID::ObsLeftRight {
                left: u.arbitrary()?,
                right: u.arbitrary()?,
            }),
            1 => {
                let s = arbitrary_string_nonempty_where(u, is_invalid_msgid_text, 'X')?;
                Ok(MessageID::Invalid(s.into()))
            }
            _ => unreachable!(),
        }
    }
}

// Must be non-empty
pub type MessageIDList<'a> = Vec<MessageID<'a>>;

impl<'a> Print for MessageIDList<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        print_seq(fmt, self, Formatter::write_fws)
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
///    our-msg-id        = our-msg-id-angle / our-msg-id-bare
///    our-msg-id-angle  = "<" our-msg-id-bare ">"
///    our-msg-id-bare   = id-left "@" id-right / 1*(not <>")
/// ```
/// The grammar above is ambiguous since "id-left @ id-right" and "1*(not <>")"
/// intersect. To work around this problem, our parsers for our-msg-id and
/// our-msg-id-bare assume that they consume all of their input. If this is not
/// the case, our-msg-id-angle should be used instead (as it is properly
/// delimited).
#[instrument_input("tracing")]
pub fn msg_id(input: &[u8]) -> IResult<&[u8], MessageID<'_>> {
    alt((
        msg_id_angle,
        map(msg_id_bare(|i: &[u8]| eof(i)), |msg| {
            #[cfg(feature = "tracing-recover")]
            warn!("message-id: bare msg-id without <>");
            msg
        }),
    ))(input)
}
pub fn msg_id_angle(input: &[u8]) -> IResult<&[u8], MessageID<'_>> {
    preceded(
        pair(opt(cfws), tag("<")),
        msg_id_bare(|i: &[u8]| recognize(pair(tag(">"), opt(cfws)))(i)),
    )(input)
}
pub fn msg_id_bare<F>(terminator: F) -> impl FnMut(&[u8]) -> IResult<&[u8], MessageID<'_>>
where
    F: for<'a> Fn(&'a [u8]) -> IResult<&'a [u8], &'a [u8]>,
{
    move |input: &[u8]| {
        alt((
            map(
                tuple((id_left, tag("@"), id_right, &terminator)),
                |(left, _, right, _)| MessageID::ObsLeftRight { left, right },
            ),
            map(
                tuple((
                    opt(cfws),
                    take_utf8_while1(is_invalid_msgid_text),
                    opt(cfws),
                    &terminator,
                )),
                |(_, s, _, _)| {
                    #[cfg(feature = "tracing-recover")]
                    warn!("message-id: bare string instead of id-left@id-right");
                    MessageID::Invalid(s)
                },
            ),
        ))(input)
    }
}

// This is VERY lenient
fn is_invalid_msgid_text(c: char) -> bool {
    is_nonascii_or(|c| c.is_ascii_graphic() && c != b'<' && c != b'>' && c != b'"')(c)
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
/// However, historical emails seem to contain a lot of nonsense in between
/// msg-id, and a lot of it is not part of the "phrase" syntax. We implement a
/// more lenient parser that skips "everything" in-between msg-ids: quoted
/// strings, encoded words (both part of the phrase syntax), and as a last
/// resort, any bytes until encountering something that could be the start of
/// one of the more "structured" tokens (msg-id, encoded word, quoted string).
///
/// Additionally, we try to recover from broken msg-ids: after reading a '<', if
/// we can't parse a valid msg-id, we skip to the next '>' and continue parsing.
#[instrument_input("tracing")]
pub fn nullable_msg_list(input: &[u8]) -> IResult<&[u8], MessageIDList<'_>> {
    let (input, tokens) = many0(alt((
        map(msg_id_angle, Some),
        // recovery: recognize a broken msg-id, skipping to the next >
        map(
            recognize(tuple((
                tag("<"),
                take_quoted_or_until(|c| c == b'>'),
                // use opt() since we might also be at end of input...
                opt(tag(">")),
            ))),
            |_i| {
                #[cfg(feature = "tracing-unsupported")]
                warn!(input = %bytes_to_trace_string(_i),
                      "unsupported msg-id in msg-list");
                None
            },
        ),
        // compliant CFWS in between msg-ids
        map(cfws, |_| None),
        // recovery: recognize junk in between msg-ids, skipping to the next <
        map(take_quoted_encoded_or_until1(|c| c == b'<'), |_i| {
            #[cfg(feature = "tracing-recover")]
            warn!(input = %bytes_to_trace_string(_i),
                  "non-compliant text between msg-ids");
            None
        }),
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
#[instrument_input("tracing")]
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
#[instrument_input("tracing")]
fn id_right(input: &[u8]) -> IResult<&[u8], Domain<'_>> {
    domain(input)
}

#[allow(dead_code)]
#[instrument_input("tracing")]
fn no_fold_literal(input: &[u8]) -> IResult<&[u8], Dtext<'_>> {
    delimited(tag("["), dtext, tag("]"))(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::imf::mailbox::{Domain, LocalPart, LocalPartToken};
    use crate::print::tests::print_to_vec;
    use crate::text::misc_token::Word;
    use crate::text::quoted::QuotedString;
    use crate::text::words::Atom;

    fn assert_msg_list_reprinted(txt: &[u8], printed: &[u8]) {
        let (rest, parsed) = nullable_msg_list(txt).unwrap();
        assert_eq!(rest, b"");
        let reprinted = print_to_vec(parsed);
        assert_eq!(
            String::from_utf8_lossy(&reprinted),
            String::from_utf8_lossy(printed)
        );
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
                    right: Domain::Atoms(vec![Atom("example".into()), Atom("com".into()),]),
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
                    right: Domain::Atoms(
                        vec![Atom("univ-valenciennes".into()), Atom("fr".into()),]
                    ),
                }
            )),
        );

        assert_eq!(
            msg_id(b"<\"24806 Tue Sep 19 11:05:34 1995\"@bnr.ca>"),
            Ok((
                &b""[..],
                MessageID::ObsLeftRight {
                    left: LocalPart(vec![LocalPartToken::Word(Word::Quoted(QuotedString(
                        vec![
                            "24806".into(),
                            " ".into(),
                            "Tue".into(),
                            " ".into(),
                            "Sep".into(),
                            " ".into(),
                            "19".into(),
                            " ".into(),
                            "11:05:34".into(),
                            " ".into(),
                            "1995".into(),
                        ]
                    )))]),
                    right: Domain::Atoms(vec![Atom("bnr".into()), Atom("ca".into()),]),
                }
            )),
        );
    }

    #[test]
    fn test_noncompliant_msg_id() {
        assert_eq!(
            msg_id(b" <523C50DA-160C-4550-A44E-7E192513CF91> "),
            Ok((
                &b""[..],
                MessageID::Invalid("523C50DA-160C-4550-A44E-7E192513CF91".into())
            ))
        );

        assert_eq!(
            msg_id(b" foo "),
            Ok((&b""[..], MessageID::Invalid("foo".into())))
        );

        assert_eq!(
            msg_id(b"text/plain.RKLqBQUAAZl1yPGCYOHKDjrj_nwwBg.1758617731@alan.eu"),
            Ok((
                &b""[..],
                MessageID::ObsLeftRight {
                    left: LocalPart(vec![
                        LocalPartToken::Word(Word::Atom(Atom("text/plain".into()))),
                        LocalPartToken::Dot,
                        LocalPartToken::Word(Word::Atom(Atom(
                            "RKLqBQUAAZl1yPGCYOHKDjrj_nwwBg".into()
                        ))),
                        LocalPartToken::Dot,
                        LocalPartToken::Word(Word::Atom(Atom("1758617731".into()))),
                    ]),
                    right: Domain::Atoms(vec![Atom("alan".into()), Atom("eu".into()),]),
                },
            ))
        );

        assert_eq!(
            msg_id(b" <aAdGYiJBX0VZF2TI@millmess@rouba.net> "),
            Ok((
                &b""[..],
                MessageID::Invalid("aAdGYiJBX0VZF2TI@millmess@rouba.net".into())
            ))
        );

        assert_eq!(
            msg_id(b"<md5:xqmIG/sV8WoSG9UzafBCGw==>"),
            Ok((
                &b""[..],
                MessageID::Invalid("md5:xqmIG/sV8WoSG9UzafBCGw==".into())
            ))
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
            b"<3AB624F9.5B6C6680@example.com>",
        );
    }

    #[test]
    fn test_msg_list_recover() {
        // The second msg-id is broken (incorrect line folding). (Found in
        // URSSAF emails.) It is parsed as MessageID::Invalid and reprinted
        // as-is. We skip it and continue parsing.
        assert_msg_list_reprinted(
            b"<abc@def>,<foo\n\tbar@outlook.com>,<baz@outlook.com>",
            b"<abc@def> <baz@outlook.com>",
        );

        // worse offenders, not found IRL but demonstrate the behavior of our
        // recovery strategy
        assert_msg_list_reprinted(b"<abc@def>,<foo\n\tbar@outlook.com ", b"<abc@def>");

        assert_msg_list_reprinted(
            b"<abc@def>,random\"garbage=?utf-8?q?aabb?= <uuu@jjj>",
            b"<abc@def> <uuu@jjj>",
        );

        assert_msg_list_reprinted(b"<abc@def>,<randomgarbage\">\" <uuu@jjj>", b"<abc@def>");
    }
}
