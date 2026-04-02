#[cfg(feature = "arbitrary")]
use arbitrary::Arbitrary;
use bounded_static::ToStatic;
use nom::{
    branch::alt,
    combinator::{all_consuming, into, map, map_opt, opt},
    bytes::complete::tag,
    multi::{many0, many1, separated_list1},
    sequence::{delimited, pair, preceded, terminated, tuple},
    IResult,
};
use std::borrow::Cow;
#[cfg(feature = "tracing")]
use tracing::warn;

#[cfg(feature = "arbitrary")]
use crate::{
    arbitrary_utils::{arbitrary_vec_nonempty, arbitrary_string_nonempty_where},
    fuzz_eq::FuzzEq,
};
#[cfg(feature = "tracing")]
use crate::utils::bytes_to_trace_string;
use crate::i18n::ContainsUtf8;
use crate::print::{print_seq, Print, Formatter, ToStringFromPrint};
use crate::text::ascii;
use crate::text::misc_token::{phrase, word, Phrase, Word, WordChars};
use crate::text::quoted::print_quoted;
use crate::text::utf8::{is_ascii_and, is_nonascii_or, take_utf8_while1};
use crate::text::whitespace::{cfws, fws, is_obs_no_ws_ctl};
use crate::text::words::{dot_atom_text, atom, Atom};

#[derive(Clone, Debug, PartialEq, ToStatic, ToStringFromPrint)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary, FuzzEq))]
pub struct AddrSpec<'a> {
    pub local_part: LocalPart<'a>,
    pub domain: Domain<'a>,
}
impl<'a> Print for AddrSpec<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        self.local_part.print(fmt);
        fmt.write_bytes(b"@");
        self.domain.print(fmt)
    }
}

#[derive(Clone, Debug, PartialEq, ToStatic, ToStringFromPrint)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary, FuzzEq))]
pub struct MailboxRef<'a> {
    // The actual "email address" like hello@example.com
    pub addrspec: AddrSpec<'a>,
    // The optional name
    pub name: Option<Phrase<'a>>,
}
impl MailboxRef<'static> {
    // Used as placeholder value for a missing or invalid address.
    // Represents "unknown@unknown".
    pub fn placeholder() -> Self {
        MailboxRef {
            addrspec: AddrSpec {
                local_part: LocalPart(vec![
                    LocalPartToken::Word(Word::Atom(Atom("unknown".into())))
                ]),
                domain: Domain::Atoms(vec![Atom("unknown".into())]),
            },
            name: None,
        }
    }
}
impl<'a> From<AddrSpec<'a>> for MailboxRef<'a> {
    fn from(addr: AddrSpec<'a>) -> Self {
        MailboxRef {
            name: None,
            addrspec: addr,
        }
    }
}
impl<'a> Print for MailboxRef<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        match &self.name {
            Some(name) => {
                name.print(fmt);
                fmt.write_fws();
                fmt.write_bytes(b"<");
                self.addrspec.print(fmt);
                fmt.write_bytes(b">")
            },
            None =>
                self.addrspec.print(fmt)
        }
    }
}

/// A non-empty list of mailboxes.
#[derive(Clone, Debug, PartialEq, ToStatic, ToStringFromPrint)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub struct MailboxList<'a>(pub Vec<MailboxRef<'a>>);

impl<'a> Print for MailboxList<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        print_seq(fmt, &self.0, |fmt| {
            fmt.write_bytes(b",");
            fmt.write_fws()
        })
    }
}
#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for MailboxList<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(MailboxList(arbitrary_vec_nonempty(u)?))
    }
}

/// Mailbox
///
/// ```abnf
///    mailbox         =   name-addr / addr-spec
/// ```
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = %bytes_to_trace_string(input)))
)]
pub fn mailbox(input: &[u8]) -> IResult<&[u8], MailboxRef<'_>> {
    alt((name_addr, into(addr_spec)))(input)
}

/// Mailbox list
///
/// ```abnf
///    mailbox-list    =   (mailbox *("," mailbox)) / obs-mbox-list
///    obs-mbox-list   =   *([CFWS] ",") mailbox *("," [mailbox / CFWS])
/// ```
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = %bytes_to_trace_string(input)))
)]
pub fn mailbox_list(input: &[u8]) -> IResult<&[u8], MailboxList<'_>> {
    map_opt(mailbox_list_nullable, |mlist| mlist)(input)
}

// mailbox-list but allows the list to only contain "null" elements
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = %bytes_to_trace_string(input)))
)]
pub(crate) fn mailbox_list_nullable(input: &[u8]) -> IResult<&[u8], Option<MailboxList<'_>>> {
    map(
        separated_list1(
            tag(","),
            alt((
                map(mailbox, Some),
                map(opt(cfws), |_| None),
            ))
        ),
        |v: Vec<Option<_>>| {
            let v: Vec<_> = v.into_iter().flatten().collect();
            if v.is_empty() {
                None
            } else {
                Some(MailboxList(v))
            }
        }
    )(input)
}

/// Name of the email address
///
/// ```abnf
///    name-addr       =   [display-name] angle-addr
/// ```
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = %bytes_to_trace_string(input)))
)]
fn name_addr(input: &[u8]) -> IResult<&[u8], MailboxRef<'_>> {
    let (input, name) = opt(phrase)(input)?;
    let (input, addrspec) = angle_addr(input)?;
    Ok((input, MailboxRef { name, addrspec }))
}

/// Enclosed addr-spec with < and >
///
/// ```abnf
/// angle-addr      =   [CFWS] "<" addr-spec ">" [CFWS] /
///                     obs-angle-addr
/// ```
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = %bytes_to_trace_string(input)))
)]
pub fn angle_addr(input: &[u8]) -> IResult<&[u8], AddrSpec<'_>> {
    delimited(
        tuple((
            opt(cfws),
            tag(&[ascii::LT]),
            opt(obs_route),
        )),
        addr_spec,
        pair(tag(&[ascii::GT]), opt(cfws)),
    )(input)
}

///    obs-route       =   obs-domain-list ":"
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = %bytes_to_trace_string(input)))
)]
fn obs_route(input: &[u8]) -> IResult<&[u8], Vec<Option<Domain<'_>>>> {
    terminated(domain_list, tag(&[ascii::COL]))(input)
}

/// Domain list.
///
/// This implement a relaxed version of the obsolete syntax:
/// ```abnf
///    obs-domain-list =   *(CFWS / ",") "@" domain
///                        *("," [CFWS] ["@" domain])
/// ```
/// The parser below is slightly more lenient as it allows domains list that
/// contain no real domains (e.g. only commas).
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = %bytes_to_trace_string(input)))
)]
fn domain_list(input: &[u8]) -> IResult<&[u8], Vec<Option<Domain<'_>>>> {
    preceded(
        opt(cfws),
        separated_list1(
            tag(&[ascii::COMMA]),
            alt((
                map(preceded(pair(opt(cfws), tag(&[ascii::AT])), domain), |d| Some(d)),
                map(opt(cfws), |_| None),
            ))
        )
    )(input)
}

/// AddrSpec
///
/// ```abnf
///    addr-spec       =   local-part "@" domain
/// ```
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = %bytes_to_trace_string(input)))
)]
pub fn addr_spec(input: &[u8]) -> IResult<&[u8], AddrSpec<'_>> {
    map(
        tuple((
            local_part,
            tag(&[ascii::AT]),
            domain,
            many0(pair(tag(&[ascii::AT]), domain)), // for compatibility reasons with ENRON
        )),
        |(local_part, _, domain, _)| AddrSpec { local_part, domain },
    )(input)
}

#[derive(Clone, Debug, PartialEq, ToStatic, ToStringFromPrint)]
pub struct LocalPart<'a>(pub Vec<LocalPartToken<'a>>); // non-empty vec

#[derive(Clone, Debug, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
pub enum LocalPartToken<'a> {
    Dot,
    Word(Word<'a>),
}

impl<'a> LocalPart<'a> {
    fn chars<'b>(&'b self) -> LocalPartChars<'a, 'b> {
        LocalPartChars { l: &self, inner: LocalPartCharsInner::NextToken(0) }
    }
}
#[derive(Clone)]
struct LocalPartChars<'a, 'b> {
    l: &'b LocalPart<'a>,
    inner: LocalPartCharsInner<'a, 'b>,
}
#[derive(Clone)]
enum LocalPartCharsInner<'a, 'b> {
    NextToken(usize),
    Word(usize, WordChars<'a, 'b>),
}
impl<'a, 'b> Iterator for LocalPartChars<'a, 'b> {
    type Item = char;
    fn next(&mut self) -> Option<Self::Item> {
        use LocalPartCharsInner::*;
        match &mut self.inner {
            NextToken(idx) =>
                match self.l.0.get(*idx) {
                    Some(LocalPartToken::Dot) => {
                        self.inner = NextToken(*idx + 1);
                        Some('.')
                    },
                    Some(LocalPartToken::Word(w)) => {
                        self.inner = Word(*idx, w.chars());
                        self.next()
                    },
                    None =>
                        None,
                },
            Word(idx, it) =>
                match it.next() {
                    Some(c) => Some(c),
                    None => {
                        self.inner = NextToken(*idx + 1);
                        self.next()
                    }
                }
        }
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for LocalPart<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(LocalPart(arbitrary_vec_nonempty(u)?))
    }
}
#[cfg(feature = "arbitrary")]
impl<'a> FuzzEq for LocalPart<'a> {
    fn fuzz_eq(&self, other: &Self) -> bool {
        self.chars().collect::<String>() == other.chars().collect::<String>()
    }
}

impl<'a> Print for LocalPart<'a> {
    // Assumption: `self.bytes()` only contains ASCII bytes.
    fn print(&self, fmt: &mut impl Formatter) {
        // Parsing of local parts is more lenient than printing (both wrt
        // the spec and because of obsolete syntax). Thus, for printing, we
        // only assume that `self` only contains ASCII and recompute how it
        // should be printed.

        // get the local part as a string
        let as_str: String = self.chars().collect();

        // If `as_str` is a dot-atom we print it as-is, otherwise
        // we quote it. This ensures that our output is compliant with RFC5322.
        if all_consuming(dot_atom_text)(as_str.as_bytes()).is_ok() {
            fmt.write_bytes(as_str.as_bytes())
        } else {
            print_quoted(fmt, self.chars())
        }
    }
}

/// Local part
///
/// Compared to the RFC, we allow multiple dots.
/// This is found in Enron emails and supported by Gmail.
/// We also allow dots at the beginning and end.
///
/// This "local part" syntax is a superset of both the RFC's
/// local-part and obs-local-part.
///
/// ```abnf
/// local-part          = dot-atom / quoted-string / obs-local-part
/// obs-local-part      = word *("." word)
/// our-local-part      =  *"." word *(1*"." word) *"."
/// ```
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = %bytes_to_trace_string(input)))
)]
fn local_part(input: &[u8]) -> IResult<&[u8], LocalPart<'_>> {
    let (input, prefix) = many0(local_part_dot)(input)?;
    let (input, w) = local_part_word(input)?;
    let (input, ws) = many0(pair(many1(local_part_dot), local_part_word))(input)?;
    let (input, suffix) = many0(local_part_dot)(input)?;

    if !prefix.is_empty() {
        #[cfg(feature = "tracing-recover")]
        warn!("best-effort local-part (leading dots)");
    }
    if !suffix.is_empty() {
        #[cfg(feature = "tracing-recover")]
        warn!("best-effort local part (trailing dots)");
    }

    let mut v: Vec<LocalPartToken> = vec![];
    v.extend(prefix);
    v.push(w);
    for (dots, w) in ws.into_iter() {
        if dots.len() > 1 {
            #[cfg(feature = "tracing-recover")]
            warn!("best-effort local part (consecutive dots)");
        }
        v.extend(dots);
        v.push(w);
    }
    v.extend(suffix);
    Ok((input, LocalPart(v)))
}
fn local_part_dot(input: &[u8]) -> IResult<&[u8], LocalPartToken<'_>> {
    map(tag(&[ascii::PERIOD]), |_| LocalPartToken::Dot)(input)
}
fn local_part_word(input: &[u8]) -> IResult<&[u8], LocalPartToken<'_>> {
    map(word, LocalPartToken::Word)(input)
}

#[derive(Clone, Debug, PartialEq, ToStatic, ToStringFromPrint)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub enum Domain<'a> {
    Atoms(Vec<Atom<'a>>), // non-empty vec
    Literal(Vec<Dtext<'a>>),
}

impl<'a> Print for Domain<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        match self {
            Domain::Atoms(atoms) => {
                print_seq(fmt, &atoms, |fmt| fmt.write_bytes(b"."))
            },
            Domain::Literal(parts) => {
                fmt.write_bytes(b"[");
                print_seq(fmt, &parts, Formatter::write_fws);
                fmt.write_bytes(b"]")
            },
        }
    }
}
#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for Domain<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        if u.arbitrary()? {
            Ok(Domain::Atoms(arbitrary_vec_nonempty(u)?))
        } else {
            Ok(Domain::Literal(u.arbitrary()?))
        }
    }
}

/// Domain
///
/// Rewritten so that domain is a superset
/// of RFC-strict domain and obs_domain.
///
/// RFC5322:
/// ```abnf
///  domain          =   dot-atom / domain-literal / obs-domain
///  obs-domain      =   atom *("." atom)
/// ```
///
/// we implement the equivalent form:
/// ```abnf
///  our-domain      = atom *("." atom) / domain-literal
/// ```
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = %bytes_to_trace_string(input)))
)]
pub fn domain(input: &[u8]) -> IResult<&[u8], Domain<'_>> {
    alt((
        map(separated_list1(tag("."), atom), Domain::Atoms),
        domain_litteral,
    ))(input)
}

/// Domain litteral
///
/// ```abnf
///    domain-literal  =   [CFWS] "[" *([FWS] dtext) [FWS] "]" [CFWS]
/// ```
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = %bytes_to_trace_string(input)))
)]
fn domain_litteral(input: &[u8]) -> IResult<&[u8], Domain<'_>> {
    delimited(
        pair(opt(cfws), tag(&[ascii::LEFT_BRACKET])),
        inner_domain_litteral,
        pair(tag(&[ascii::RIGHT_BRACKET]), opt(cfws)),
    )(input)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = %bytes_to_trace_string(input)))
)]
fn inner_domain_litteral(input: &[u8]) -> IResult<&[u8], Domain<'_>> {
    map(
        terminated(many0(preceded(opt(fws), dtext)), opt(fws)),
        Domain::Literal
    )(input)
}

// Invariant: must be non-empty
#[derive(Clone, ContainsUtf8, Debug, PartialEq, ToStatic, ToStringFromPrint)]
pub struct Dtext<'a>(Cow<'a, str>);

impl<'a> Dtext<'a> {
    // Best-effort conversion of any `Dtext` contents into chars that all
    // satisfy `is_strict_dtext`.
    //
    // - We drop characters which are not part of the strict syntax.
    // Unfortunately this can drop printable characters, if they were part
    // of a quote (\X), which is accepted by the obsolete syntax. However,
    // we have no better option than to drop those since there is no way
    // to represent them in the strict syntax.
    // - Dropping obsolete characters may result in an empty string; however
    // a `Dtext` must always be nonempty; in this case, we return "?", as a
    // placeholder text.
    // XXX it would be more consistent with the rest of the codebase if this
    // sanitization was done at parsing time, resulting in an AST which is
    // always "clean" as an invariant and can be printed directly.
    fn to_strict_best_effort(&self) -> Self {
        let mut strict_dtext: String =
            self.0.chars().filter(|c| is_strict_dtext(*c)).collect();
        if strict_dtext.is_empty() {
            strict_dtext.push('?')
        }
        Dtext(strict_dtext.into())
    }
}

impl<'a> Print for Dtext<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        fmt.write_bytes(&self.to_strict_best_effort().0.as_bytes())
    }
}
#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for Dtext<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Dtext<'a>> {
        let s: String = arbitrary_string_nonempty_where(u, is_dtext, 'X')?;
        Ok(Dtext(Cow::Owned(s)))
    }
}
#[cfg(feature = "arbitrary")]
impl<'a> FuzzEq for Dtext<'a> {
    fn fuzz_eq(&self, other: &Self) -> bool {
        self.to_strict_best_effort() == other.to_strict_best_effort()
    }
}

/// Is domain text
///
/// ```abnf
///   dtext           =   %d33-90 /          ; Printable US-ASCII
///                       %d94-126 /         ;  characters not including
///                       obs-dtext          ;  "[", "]", or "\"
///   obs-dtext       =   obs-NO-WS-CTL / quoted-pair
/// ```
/// following RFC6532, also allows non-ascii UTF-8 text
fn is_dtext(c: char) -> bool {
    is_strict_dtext(c) || is_obs_dtext(c)
}
fn is_strict_dtext(c: char) -> bool {
    is_nonascii_or(|c| {
        (0x21..=0x5A).contains(&c) || (0x5E..=0x7E).contains(&c)
    })(c)
}
fn is_obs_dtext(c: char) -> bool {
    is_ascii_and(is_obs_no_ws_ctl)(c)
    //@FIXME does not support quoted pair yet while RFC requires it
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = %bytes_to_trace_string(input)))
)]
pub fn dtext<'a>(input: &'a [u8]) -> IResult<&'a [u8], Dtext<'a>> {
    map(take_utf8_while1(is_dtext), Dtext)(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::print::tests::print_to_vec;
    use crate::text::misc_token::PhraseToken;
    use crate::text::quoted::QuotedString;

    // NOTE: this roundtrip property does not hold in general for all valid
    // 'addr-spec's, in particular because of the obsolete syntax (which gets
    // dropped when printed back) but also because of quoting ('\a' gets printed
    // back as 'a').
    fn addr_roundtrip_as(addr: &[u8], parsed: AddrSpec<'_>) {
        assert_eq!(addr_spec(addr), Ok((&b""[..], parsed.clone())));
        let printed = print_to_vec(parsed);
        assert_eq!(String::from_utf8_lossy(addr), String::from_utf8_lossy(&printed));
    }
    fn addr_roundtrip(addr: &[u8]) {
        let (input, parsed) = addr_spec(addr).unwrap();
        assert!(input.is_empty());
        let printed = print_to_vec(parsed);
        assert_eq!(String::from_utf8_lossy(addr), String::from_utf8_lossy(&printed));
    }
    fn addr_parsed_printed(addr: &[u8], parsed: AddrSpec<'_>, printed: &[u8]) {
        assert_eq!(addr_spec(addr), Ok((&b""[..], parsed.clone())));
        let reprinted = print_to_vec(parsed);
        assert_eq!(String::from_utf8_lossy(printed), String::from_utf8_lossy(&reprinted));
    }

    // NOTE: like for addr-spec, this roundtrip property is not expected to hold
    // in general.
    fn mailbox_roundtrip_as(mbox: &[u8], parsed: MailboxRef<'_>) {
        assert_eq!(mailbox(mbox), Ok((&b""[..], parsed.clone())));
        let printed = print_to_vec(parsed);
        assert_eq!(String::from_utf8_lossy(mbox), String::from_utf8_lossy(&printed));
    }
    fn mailbox_parsed_printed(mbox: &[u8], parsed: MailboxRef<'_>, printed: &[u8]) {
        assert_eq!(mailbox(mbox), Ok((&b""[..], parsed.clone())));
        let reprinted = print_to_vec(parsed);
        assert_eq!(String::from_utf8_lossy(printed), String::from_utf8_lossy(&reprinted));
    }

    fn mailbox_list_reprint(mboxlist: &[u8], printed: &[u8]) {
        let (input, parsed) = mailbox_list(mboxlist).unwrap();
        assert!(input.is_empty());
        let reprinted = print_to_vec(parsed);
        assert_eq!(String::from_utf8_lossy(&reprinted), String::from_utf8_lossy(printed));
    }

    #[test]
    fn test_addr_spec() {
        addr_roundtrip_as(
            b"alice@example.com",
            AddrSpec {
                local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(Atom("alice"[..].into())))]),
                domain: Domain::Atoms(vec![Atom("example"[..].into()), Atom("com"[..].into())]),
            }
        );

        addr_roundtrip_as(
            b"alice@smtp.example.com",
            AddrSpec {
                local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(Atom("alice"[..].into())))]),
                domain: Domain::Atoms(vec![
                    Atom("smtp"[..].into()),
                    Atom("example"[..].into()),
                    Atom("com"[..].into()),
                ]),
            }
        );

        addr_roundtrip_as(
            b"jsmith@[192.168.2.1]",
            AddrSpec {
                local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(Atom("jsmith"[..].into())))]),
                domain: Domain::Literal(vec![Dtext("192.168.2.1".into())]),
            }
        );

        addr_roundtrip_as(
            b"jsmith@[IPv6:2001:db8::1]",
            AddrSpec {
                local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(Atom("jsmith"[..].into())))]),
                domain: Domain::Literal(vec![Dtext("IPv6:2001:db8::1".into())]),
            }
        );

        // UTF-8
        addr_roundtrip_as(
            "用户@例子.广告".as_bytes(),
            AddrSpec {
                local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(Atom("用户".into())))]),
                domain: Domain::Atoms(vec![
                    Atom("例子".into()),
                    Atom("广告".into()),
                ]),
            }
        );

        // ASCII Edge cases
        addr_roundtrip(b"user+mailbox/department=shipping@example.com");
        addr_roundtrip(b"!#$%&'*+-/=?^_`.{|}~@example.com");

        addr_roundtrip_as(
            r#""Abc@def"@example.com"#.as_bytes(),
            AddrSpec {
                local_part: LocalPart(vec![LocalPartToken::Word(Word::Quoted(QuotedString(
                    vec!["Abc@def".into()]
                )))]),
                domain: Domain::Atoms(vec![Atom("example"[..].into()), Atom("com"[..].into())]),
            }
        );
        addr_parsed_printed(
            r#""Fred\ Bloggs"@example.com"#.as_bytes(),
            AddrSpec {
                local_part: LocalPart(vec![LocalPartToken::Word(Word::Quoted(QuotedString(
                    vec!["Fred".into(), " ".into(), "Bloggs".into()]
                )))]),
                domain: Domain::Atoms(vec![Atom("example"[..].into()), Atom("com"[..].into())]),
            },
            r#""Fred Bloggs"@example.com"#.as_bytes(), // escaping the space is unnecessary
        );
        addr_roundtrip_as(
            r#""Joe.\\Blow"@example.com"#.as_bytes(),
            AddrSpec {
                local_part: LocalPart(vec![LocalPartToken::Word(Word::Quoted(QuotedString(
                    vec!["Joe.".into(), "\\".into(), "Blow".into()]
                )))]),
                domain: Domain::Atoms(vec![Atom("example"[..].into()), Atom("com"[..].into())]),
            }
        );

        // edge-case: domain literal part that contains only obsolete bytes -> gets reprinted as '?'
        let mut addr = b"foobar@[X ".to_vec();
        addr.extend(&[1, 0x1c, b']']);
        addr_parsed_printed(
            &addr,
            AddrSpec {
                local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(Atom("foobar".into())))]),
                domain: Domain::Literal(vec![
                    Dtext("X"[..].into()),
                    Dtext("\x01\x1c".into()),
                ]),
            },
            b"foobar@[X ?]",
        );
    }

    #[test]
    fn test_gmail_noncompliant() {
        addr_parsed_printed(
            b"foo..bar@gmail.com",
            AddrSpec {
                local_part: LocalPart(vec![
                    LocalPartToken::Word(Word::Atom(Atom("foo".into()))),
                    LocalPartToken::Dot,
                    LocalPartToken::Dot,
                    LocalPartToken::Word(Word::Atom(Atom("bar".into()))),
                ]),
                domain: Domain::Atoms(vec![Atom("gmail"[..].into()), Atom("com"[..].into())]),
            },
            b"\"foo..bar\"@gmail.com",
        )
    }

    #[test]
    fn test_mailbox() {
        mailbox_roundtrip_as(
            r#""Joe Q. Public" <john.q.public@example.com>"#.as_bytes(),
            MailboxRef {
                name: Some(Phrase(vec![
                    PhraseToken::Word(Word::Quoted(QuotedString(vec![
                        "Joe"[..].into(),
                        " ".into(),
                        "Q."[..].into(),
                        " ".into(),
                        "Public"[..].into(),
                    ])))])),
                addrspec: AddrSpec {
                    local_part: LocalPart(vec![
                        LocalPartToken::Word(Word::Atom(Atom("john"[..].into()))),
                        LocalPartToken::Dot,
                        LocalPartToken::Word(Word::Atom(Atom("q"[..].into()))),
                        LocalPartToken::Dot,
                        LocalPartToken::Word(Word::Atom(Atom("public"[..].into()))),
                    ]),
                    domain: Domain::Atoms(vec![Atom("example"[..].into()), Atom("com"[..].into())]),
                }
            }
        );

        // UTF-8 with invalid bytes
        assert_eq!(
            mailbox(b"a\xD4\xC6z\xE7 <tigermeeting@mail.net>"),
            Ok((&b""[..],
                MailboxRef {
                    name: Some(Phrase(vec![
                        PhraseToken::Word(Word::Atom(Atom("a\u{FFFD}\u{FFFD}z\u{FFFD}".into()))),
                    ])),
                    addrspec: AddrSpec {
                        local_part: LocalPart(vec![
                            LocalPartToken::Word(Word::Atom(Atom("tigermeeting".into())))
                        ]),
                        domain: Domain::Atoms(vec![Atom("mail".into()), Atom("net".into())]),
                    },
                }
            ))
        );

        mailbox_roundtrip_as(
            r#"Mary Smith <mary@x.test>"#.as_bytes(),
            MailboxRef {
                name: Some(Phrase(vec![
                    PhraseToken::Word(Word::Atom(Atom("Mary"[..].into()))),
                    PhraseToken::Word(Word::Atom(Atom("Smith"[..].into())))
                ])),
                addrspec: AddrSpec {
                    local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(Atom("mary"[..].into())))]),
                    domain: Domain::Atoms(vec![Atom("x"[..].into()), Atom("test"[..].into())]),
                }
            }
        );

        mailbox_roundtrip_as(
            r#"jdoe@example.org"#.as_bytes(),
            MailboxRef {
                name: None,
                addrspec: AddrSpec {
                    local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(Atom("jdoe"[..].into())))]),
                    domain: Domain::Atoms(vec![Atom("example"[..].into()), Atom("org"[..].into())]),
                }
            }
        );

        mailbox_roundtrip_as(
            r#"Who? <one@y.test>"#.as_bytes(),
            MailboxRef {
                name: Some(Phrase(vec![PhraseToken::Word(Word::Atom(Atom("Who?"[..].into())))])),
                addrspec: AddrSpec {
                    local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(Atom("one"[..].into())))]),
                    domain: Domain::Atoms(vec![Atom("y"[..].into()), Atom("test"[..].into())]),
                }
            }
        );

        mailbox_parsed_printed(
            r#"<boss@nil.test>"#.as_bytes(),
            MailboxRef {
                name: None,
                addrspec: AddrSpec {
                    local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(Atom("boss"[..].into())))]),
                    domain: Domain::Atoms(vec![Atom("nil"[..].into()), Atom("test"[..].into())]),
                }
            },
            r#"boss@nil.test"#.as_bytes(),
        );

        mailbox_roundtrip_as(
            r#""Giant; \"Big\" Box" <sysservices@example.net>"#.as_bytes(),
            MailboxRef {
                name: Some(Phrase(vec![
                    PhraseToken::Word(Word::Quoted(QuotedString(vec![
                        "Giant;"[..].into(),
                        " ".into(),
                        "\"".into(),
                        "Big"[..].into(),
                        "\"".into(),
                        " ".into(),
                        "Box"[..].into()
                    ])))])),
                addrspec: AddrSpec {
                    local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(
                        Atom("sysservices"[..].into())
                    ))]),
                    domain: Domain::Atoms(vec![Atom("example"[..].into()), Atom("net"[..].into())]),
                }
            }
        );

        // Tricky example illustrating a subtility of parsing encoded words.
        // A mailbox can start with a phrase, which allows encoded words.
        // However, "=?X?q?@[?=" *IS NOT* a valid encoded word in a phrase (because of '@' and '['),
        // even though it is a valid encoded word in other contexts.
        // This means that the correct way to parse this input is as an addr-spec...
        mailbox_roundtrip_as(
            r#"=?X?q?@[?= <?@?>]"#.as_bytes(),
            MailboxRef {
                name: None,
                addrspec: AddrSpec {
                    local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(
                        Atom("=?X?q?"[..].into())
                    ))]),
                    domain: Domain::Literal(vec![
                        Dtext("?="[..].into()),
                        Dtext("<?@?>"[..].into()),
                    ]),
                },
            },
        );
    }

    #[test]
    fn test_domain_list() {
        assert_eq!(
            domain_list(
                r#"(shhh it's coming)
 ,
 (not yet)
 @33+4.com,,,,
 ,,,,
 (again)
 @example.com,@yep.com,@a,@b,,,@c"#
                    .as_bytes()
            ),
            Ok((
                &b""[..],
                vec![
                    None,
                    Some(Domain::Atoms(vec![Atom("33+4"[..].into()), Atom("com"[..].into())])),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(Domain::Atoms(vec![Atom("example"[..].into()), Atom("com"[..].into())])),
                    Some(Domain::Atoms(vec![Atom("yep"[..].into()), Atom("com"[..].into())])),
                    Some(Domain::Atoms(vec![Atom("a"[..].into())])),
                    Some(Domain::Atoms(vec![Atom("b"[..].into())])),
                    None,
                    None,
                    Some(Domain::Atoms(vec![Atom("c"[..].into())])),
                ]
            ))
        );

        assert_eq!(
            domain_list(b",, ,@foo,"),
            Ok((
                &b""[..],
                vec![
                    None,
                    None,
                    None,
                    Some(Domain::Atoms(vec![Atom("foo"[..].into())])),
                    None,
                ]
            ))
        );
    }

    #[test]
    fn test_enron1() {
        addr_parsed_printed(
            "a..howard@enron.com".as_bytes(),
            AddrSpec {
                local_part: LocalPart(vec![
                    LocalPartToken::Word(Word::Atom(Atom("a"[..].into()))),
                    LocalPartToken::Dot,
                    LocalPartToken::Dot,
                    LocalPartToken::Word(Word::Atom(Atom("howard"[..].into()))),
                ]),
                domain: Domain::Atoms(vec![Atom("enron"[..].into()), Atom("com"[..].into())]),
            },
            r#""a..howard"@enron.com"#.as_bytes()
        );
    }

    #[test]
    fn test_enron2() {
        addr_parsed_printed(
            ".nelson@enron.com".as_bytes(),
            AddrSpec {
                local_part: LocalPart(vec![
                    LocalPartToken::Dot,
                    LocalPartToken::Word(Word::Atom(Atom("nelson"[..].into()))),
                ]),
                domain: Domain::Atoms(vec![Atom("enron"[..].into()), Atom("com"[..].into())]),
            },
            r#"".nelson"@enron.com"#.as_bytes(),
        );
    }

    #[test]
    fn test_enron3() {
        addr_parsed_printed(
            "ecn2760.conf.@enron.com".as_bytes(),
            AddrSpec {
                local_part: LocalPart(vec![
                    LocalPartToken::Word(Word::Atom(Atom("ecn2760"[..].into()))),
                    LocalPartToken::Dot,
                    LocalPartToken::Word(Word::Atom(Atom("conf"[..].into()))),
                    LocalPartToken::Dot,
                ]),
                domain: Domain::Atoms(vec![Atom("enron"[..].into()), Atom("com"[..].into())]),
            },
            r#""ecn2760.conf."@enron.com"#.as_bytes(),
        );
    }

    #[test]
    fn test_enron4() {
        mailbox_parsed_printed(
            r#"<"mark_kopinski/intl/acim/americancentury"@americancentury.com@enron.com>"#
                .as_bytes(),
            MailboxRef {
                name: None,
                addrspec: AddrSpec {
                    local_part: LocalPart(vec![LocalPartToken::Word(Word::Quoted(
                        QuotedString(vec!["mark_kopinski/intl/acim/americancentury"[..].into(),])
                    ))]),
                    domain: Domain::Atoms(vec![Atom("americancentury"[..].into()), Atom("com"[..].into())]),
                }
            },
            b"mark_kopinski/intl/acim/americancentury@americancentury.com",
        );
    }

    #[test]
    fn test_mailbox_list() {
        mailbox_list_reprint(
            r#"Pete(A nice \) chap) <pete(his account)@silly.test(his host)>"#.as_bytes(),
            b"Pete <pete@silly.test>",
        );

        mailbox_list_reprint(
            r#"Mary Smith <mary@x.test>, jdoe@example.org, Who? <one@y.test>, <boss@nil.test>, "Giant; \"Big\" Box" <sysservices@example.net>"#.as_bytes(),
            r#"Mary Smith <mary@x.test>, jdoe@example.org, Who? <one@y.test>, boss@nil.test, "Giant; \"Big\" Box" <sysservices@example.net>"#.as_bytes(),
        );
    }

    #[test]
    fn test_mailbox_list_obs() {
        mailbox_list_reprint(
            b",foo@bar.com,,boss@nil.test,jdoe@example.org, \r\n ,,",
            br#"foo@bar.com, boss@nil.test, jdoe@example.org"#,
        );
    }

    #[test]
    fn test_dtext_strictify() {
        let s: &str = &Dtext("\x03".into()).to_strict_best_effort().0;
        assert_eq!(s, "?")
    }
}
