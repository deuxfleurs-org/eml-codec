use bounded_static::ToStatic;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_while1},
    combinator::{all_consuming, into, map, opt},
    multi::{many0, separated_list1},
    sequence::{delimited, pair, preceded, terminated, tuple},
    IResult,
    Parser,
};
use std::borrow::Cow;
use std::fmt;

use crate::display_bytes::{print_seq, Print, Formatter};
use crate::text::ascii;
use crate::text::misc_token::{phrase, word, Phrase, Word};
use crate::text::quoted::print_quoted;
use crate::text::whitespace::{cfws, fws, is_obs_no_ws_ctl};
use crate::text::words::{dot_atom_text, atom};

#[derive(Clone, PartialEq, ToStatic)]
pub struct AddrSpec<'a> {
    pub local_part: LocalPart<'a>,
    pub domain: Domain<'a>,
}
impl<'a> ToString for AddrSpec<'a> {
    fn to_string(&self) -> String {
        format!(
            "{}@{}",
            self.local_part.to_string(),
            self.domain.to_string()
        )
    }
}
impl<'a> fmt::Debug for AddrSpec<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_tuple("AddrSpec")
            .field(&format_args!("\"{}\"", self.to_string()))
            .finish()
    }
}
impl<'a> Print for AddrSpec<'a> {
    fn print(&self, fmt: &mut impl Formatter) -> std::io::Result<()> {
        self.local_part.print(fmt)?;
        fmt.write_bytes(b"@")?;
        self.domain.print(fmt)
    }
}

#[derive(Clone, Debug, PartialEq, ToStatic)]
pub struct MailboxRef<'a> {
    // The actual "email address" like hello@example.com
    pub addrspec: AddrSpec<'a>,
    pub name: Option<Phrase<'a>>,
}
impl<'a> ToString for MailboxRef<'a> {
    fn to_string(&self) -> String {
        match &self.name {
            Some(n) => format!("{} <{}>", n.to_string(), self.addrspec.to_string()),
            None => self.addrspec.to_string(),
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
    fn print(&self, fmt: &mut impl Formatter) -> std::io::Result<()> {
        match &self.name {
            Some(name) => {
                name.print(fmt)?;
                fmt.write_fws()?;
                fmt.write_bytes(b"<")?;
                self.addrspec.print(fmt)?;
                fmt.write_bytes(b">")
            },
            None =>
                self.addrspec.print(fmt)
        }
    }
}

pub type MailboxList<'a> = Vec<MailboxRef<'a>>;

impl<'a> Print for MailboxList<'a> {
    fn print(&self, fmt: &mut impl Formatter) -> std::io::Result<()> {
        print_seq(fmt, self, |fmt| {
            fmt.write_bytes(b",")?;
            fmt.write_fws()
        })
    }
}

/// Mailbox
///
/// ```abnf
///    mailbox         =   name-addr / addr-spec
/// ```
pub fn mailbox(input: &[u8]) -> IResult<&[u8], MailboxRef<'_>> {
    alt((name_addr, into(addr_spec)))(input)
}

/// Name of the email address
///
/// ```abnf
///    name-addr       =   [display-name] angle-addr
/// ```
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
pub fn angle_addr(input: &[u8]) -> IResult<&[u8], AddrSpec<'_>> {
    delimited(
        tuple((opt(cfws), tag(&[ascii::LT]), opt(obs_route))),
        addr_spec,
        pair(tag(&[ascii::GT]), opt(cfws)),
    )(input)
}

///    obs-route       =   obs-domain-list ":"
fn obs_route(input: &[u8]) -> IResult<&[u8], Vec<Option<Domain<'_>>>> {
    terminated(obs_domain_list, tag(&[ascii::COL]))(input)
}

/// ```abnf
///    obs-domain-list =   *(CFWS / ",") "@" domain
///                        *("," [CFWS] ["@" domain])
/// ```
/// The parser below is slightly more lenient as it allows domains list that
/// contain no real domains (e.g. only commas).
fn obs_domain_list(input: &[u8]) -> IResult<&[u8], Vec<Option<Domain<'_>>>> {
    preceded(
        opt(cfws),
        separated_list1(
            tag(&[ascii::COMMA]),
            alt((
                map(preceded(pair(opt(cfws), tag(&[ascii::AT])), obs_domain), |d| Some(d)),
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
pub fn addr_spec(input: &[u8]) -> IResult<&[u8], AddrSpec<'_>> {
    map(
        tuple((
            obs_local_part,
            tag(&[ascii::AT]),
            obs_domain,
            many0(pair(tag(&[ascii::AT]), obs_domain)), // for compatibility reasons with ENRON
        )),
        |(local_part, _, domain, _)| AddrSpec { local_part, domain },
    )(input)
}

#[derive(Clone, Debug, PartialEq, ToStatic)]
pub enum LocalPartToken<'a> {
    Dot,
    Word(Word<'a>),
}

#[derive(Clone, Debug, PartialEq, ToStatic)]
pub struct LocalPart<'a>(pub Vec<LocalPartToken<'a>>);

impl<'a> LocalPart<'a> {
    pub fn to_string(&self) -> String {
        self.0.iter().fold(String::new(), |mut acc, token| {
            match token {
                LocalPartToken::Dot => acc.push('.'),
                LocalPartToken::Word(v) => acc.push_str(v.to_string().as_ref()),
            }
            acc
        })
    }
}

impl<'a> Print for LocalPart<'a> {
    // Assumption: `self.bytes()` only contains ASCII bytes.
    fn print(&self, fmt: &mut impl Formatter) -> std::io::Result<()> {
        // Parsing of local parts is more lenient than printing (both wrt
        // the spec and because of obsolete syntax). Thus, for printing, we
        // only assume that `self` only contains ASCII and recompute how it
        // should be printed.

        // print the local part as raw bytes
        let as_bytes: Vec<u8> = {
            let mut v = Vec::new();
            for tok in &self.0 {
                match tok {
                    LocalPartToken::Dot => v.push(b'.'),
                    LocalPartToken::Word(w) => v.extend(w.bytes()),
                }
            }
            v
        };

        // If `as_bytes` is a dot-atom we print it as-is, otherwise
        // we quote it. This ensures that our output is compliant with RFC5322.
        if all_consuming(dot_atom_text)(&as_bytes).is_ok() {
            fmt.write_bytes(&as_bytes)
        } else {
            print_quoted(fmt, as_bytes.iter().copied())
        }
    }
}

/// Obsolete local part
///
/// Compared to the RFC, we allow multiple dots.
/// This is found in Enron emails and supported by Gmail.
///
/// Obsolete local part is a superset of strict_local_part:
/// anything that is parsed by strict_local_part will be parsed by
/// obs_local_part.
///
/// ```abnf
/// obs-local-part  =  *("." / word)
/// ```
fn obs_local_part(input: &[u8]) -> IResult<&[u8], LocalPart<'_>> {
    map(
        many0(alt((
            map(tag(&[ascii::PERIOD]), |_| LocalPartToken::Dot),
            map(word, LocalPartToken::Word),
        ))),
        LocalPart,
    )(input)
}

#[derive(Clone, PartialEq, ToStatic)]
pub enum Domain<'a> {
    Atoms(Vec<Cow<'a, [u8]>>),
    Literal(Vec<Dtext<'a>>),
}

impl<'a> ToString for Domain<'a> {
    fn to_string(&self) -> String {
        match self {
            Domain::Atoms(v) => v
                .iter()
                .map(|v| {
                    encoding_rs::UTF_8
                        .decode_without_bom_handling(v)
                        .0
                        .to_string()
                })
                .collect::<Vec<String>>()
                .join("."),
            Domain::Literal(v) => {
                let inner = v
                    .iter()
                    .map(|dt| dt.to_string())
                    .collect::<Vec<String>>()
                    .join(" ");
                format!("[{}]", inner)
            }
        }
    }
}
impl<'a> fmt::Debug for Domain<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_tuple("Domain")
            .field(&format_args!("\"{}\"", self.to_string()))
            .finish()
    }
}

impl<'a> Print for Domain<'a> {
    fn print(&self, fmt: &mut impl Formatter) -> std::io::Result<()> {
        match self {
            Domain::Atoms(atoms) => {
                print_seq(fmt, &atoms, |fmt| fmt.write_bytes(b"."))
            },
            Domain::Literal(parts) => {
                fmt.write_bytes(b"[")?;
                print_seq(fmt, &parts, Formatter::write_fws)?;
                fmt.write_bytes(b"]")
            },
        }
    }
}

/// Obsolete domain
///
/// Rewritten so that obs_domain is a superset
/// of strict_domain.
///
/// RFC5322:
/// ```abnf
///  domain          =   dot-atom / domain-literal / obs-domain
///  obs-domain      =   atom *("." atom)
/// ```
///
/// we implement the equivalent form:
/// ```abnf
///  obs-domain      = atom *("." atom) / domain-literal
/// ```
pub fn obs_domain(input: &[u8]) -> IResult<&[u8], Domain<'_>> {
    alt((
        map(separated_list1(tag("."), atom.map(Cow::Borrowed)), Domain::Atoms),
        domain_litteral,
    ))(input)
}

/// Domain litteral
///
/// ```abnf
///    domain-literal  =   [CFWS] "[" *([FWS] dtext) [FWS] "]" [CFWS]
/// ```
fn domain_litteral(input: &[u8]) -> IResult<&[u8], Domain<'_>> {
    delimited(
        pair(opt(cfws), tag(&[ascii::LEFT_BRACKET])),
        inner_domain_litteral,
        pair(tag(&[ascii::RIGHT_BRACKET]), opt(cfws)),
    )(input)
}

fn inner_domain_litteral(input: &[u8]) -> IResult<&[u8], Domain<'_>> {
    map(
        terminated(many0(preceded(opt(fws), dtext)), opt(fws)),
        Domain::Literal
    )(input)
}

#[derive(Clone, PartialEq, ToStatic)]
pub struct Dtext<'a>(Cow<'a, [u8]>);

impl<'a> ToString for Dtext<'a> {
    fn to_string(&self) -> String {
        encoding_rs::UTF_8
            .decode_without_bom_handling(&self.0)
            .0
            .to_string()
    }
}
impl<'a> fmt::Debug for Dtext<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_tuple("Dtext")
            .field(&format_args!("\"{}\"", self.to_string()))
            .finish()
    }
}

impl<'a> Print for Dtext<'a> {
    fn print(&self, fmt: &mut impl Formatter) -> std::io::Result<()> {
        for &b in self.0.iter() {
            // NOTE: we drop characters which are not part of the strict syntax.
            // Unfortunately this can drop printable characters, if they were part
            // of a quote (\X), which is accepted by the obsolete syntax. However,
            // we have no better option than to drop those since there is no way
            // to represent them in the strict syntax.
            if is_strict_dtext(b) {
                fmt.write_bytes(&[b])?;
            }
        }
        Ok(())
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
fn is_dtext(c: u8) -> bool {
    is_strict_dtext(c) || is_obs_dtext(c)
}
fn is_strict_dtext(c: u8) -> bool {
    (0x21..=0x5A).contains(&c) || (0x5E..=0x7E).contains(&c)
}
fn is_obs_dtext(c: u8) -> bool {
    is_obs_no_ws_ctl(c)
    //@FIXME does not support quoted pair yet while RFC requires it
}

pub fn dtext<'a>(input: &'a [u8]) -> IResult<&'a [u8], Dtext<'a>> {
    map(take_while1(is_dtext), |b| Dtext(Cow::Borrowed(b)))(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::misc_token::PhraseToken;
    use crate::text::quoted::QuotedString;

    // NOTE: this roundtrip property does not hold in general for all valid
    // 'addr-spec's, in particular because of the obsolete syntax (which gets
    // dropped when printed back) but also because of quoting ('\a' gets printed
    // back as 'a').
    fn addr_roundtrip_as(addr: &[u8], parsed: AddrSpec<'_>) {
        assert_eq!(addr_spec(addr), Ok((&b""[..], parsed.clone())));
        let mut v = Vec::new();
        parsed.print(&mut v).unwrap();
        assert_eq!(String::from_utf8_lossy(addr), String::from_utf8_lossy(&v));
    }
    fn addr_roundtrip(addr: &[u8]) {
        let (input, parsed) = addr_spec(addr).unwrap();
        assert!(input.is_empty());
        let mut v = Vec::new();
        parsed.print(&mut v).unwrap();
        assert_eq!(String::from_utf8_lossy(addr), String::from_utf8_lossy(&v));
    }
    fn addr_parsed_printed(addr: &[u8], parsed: AddrSpec<'_>, printed: &[u8]) {
        assert_eq!(addr_spec(addr), Ok((&b""[..], parsed.clone())));
        let mut v = Vec::new();
        parsed.print(&mut v).unwrap();
        assert_eq!(String::from_utf8_lossy(printed), String::from_utf8_lossy(&v));
    }

    // NOTE: like for addr-spec, this roundtrip property is not expected to hold
    // in general.
    fn mailbox_roundtrip_as(mbox: &[u8], parsed: MailboxRef<'_>) {
        assert_eq!(mailbox(mbox), Ok((&b""[..], parsed.clone())));
        let mut v = Vec::new();
        parsed.print(&mut v).unwrap();
        assert_eq!(String::from_utf8_lossy(mbox), String::from_utf8_lossy(&v));
    }
    fn mailbox_parsed_printed(mbox: &[u8], parsed: MailboxRef<'_>, printed: &[u8]) {
        assert_eq!(mailbox(mbox), Ok((&b""[..], parsed.clone())));
        let mut v = Vec::new();
        parsed.print(&mut v).unwrap();
        assert_eq!(String::from_utf8_lossy(printed), String::from_utf8_lossy(&v));
    }

    #[test]
    fn test_addr_spec() {
        addr_roundtrip_as(
            b"alice@example.com",
            AddrSpec {
                local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(b"alice"[..].into()))]),
                domain: Domain::Atoms(vec![b"example"[..].into(), b"com"[..].into()]),
            }
        );

        addr_roundtrip_as(
            b"jsmith@[192.168.2.1]",
            AddrSpec {
                local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(b"jsmith"[..].into()))]),
                domain: Domain::Literal(vec![Dtext(b"192.168.2.1".into())]),
            }
        );

        addr_roundtrip_as(
            b"jsmith@[IPv6:2001:db8::1]",
            AddrSpec {
                local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(b"jsmith"[..].into()))]),
                domain: Domain::Literal(vec![Dtext(b"IPv6:2001:db8::1".into())]),
            }
        );

        // UTF-8
        // @FIXME ASCII SUPPORT IS BROKEN
        /*assert_eq!(
            addr_spec("用户@例子.广告"),
            Ok((
                "",
                AddrSpec {
                    local_part: "用户".into(),
                    domain: "例子.广告".into()
                }
            ))
        );*/

        // ASCII Edge cases
        addr_roundtrip(b"user+mailbox/department=shipping@example.com");
        addr_roundtrip(b"!#$%&'*+-/=?^_`.{|}~@example.com");

        addr_roundtrip_as(
            r#""Abc@def"@example.com"#.as_bytes(),
            AddrSpec {
                local_part: LocalPart(vec![LocalPartToken::Word(Word::Quoted(QuotedString(
                    vec![b"Abc@def".into()]
                )))]),
                domain: Domain::Atoms(vec![b"example"[..].into(), b"com"[..].into()]),
            }
        );
        addr_parsed_printed(
            r#""Fred\ Bloggs"@example.com"#.as_bytes(),
            AddrSpec {
                local_part: LocalPart(vec![LocalPartToken::Word(Word::Quoted(QuotedString(
                    vec![b"Fred".into(), b" ".into(), b"Bloggs".into()]
                )))]),
                domain: Domain::Atoms(vec![b"example"[..].into(), b"com"[..].into()]),
            },
            r#""Fred Bloggs"@example.com"#.as_bytes(), // escaping the space is unnecessary
        );
        addr_roundtrip_as(
            r#""Joe.\\Blow"@example.com"#.as_bytes(),
            AddrSpec {
                local_part: LocalPart(vec![LocalPartToken::Word(Word::Quoted(QuotedString(
                    vec![b"Joe.".into(), vec![ascii::BACKSLASH].into(), b"Blow".into()]
                )))]),
                domain: Domain::Atoms(vec![b"example"[..].into(), b"com"[..].into()]),
            }
        );
    }

    #[test]
    fn test_mailbox() {
        mailbox_roundtrip_as(
            r#""Joe Q. Public" <john.q.public@example.com>"#.as_bytes(),
            MailboxRef {
                name: Some(Phrase(vec![
                    PhraseToken::Word(Word::Quoted(QuotedString(vec![
                        b"Joe"[..].into(),
                        vec![ascii::SP].into(),
                        b"Q."[..].into(),
                        vec![ascii::SP].into(),
                        b"Public"[..].into(),
                    ])))])),
                addrspec: AddrSpec {
                    local_part: LocalPart(vec![
                        LocalPartToken::Word(Word::Atom(b"john"[..].into())),
                        LocalPartToken::Dot,
                        LocalPartToken::Word(Word::Atom(b"q"[..].into())),
                        LocalPartToken::Dot,
                        LocalPartToken::Word(Word::Atom(b"public"[..].into())),
                    ]),
                    domain: Domain::Atoms(vec![b"example"[..].into(), b"com"[..].into()]),
                }
            }
        );

        mailbox_roundtrip_as(
            r#"Mary Smith <mary@x.test>"#.as_bytes(),
            MailboxRef {
                name: Some(Phrase(vec![
                    PhraseToken::Word(Word::Atom(b"Mary"[..].into())),
                    PhraseToken::Word(Word::Atom(b"Smith"[..].into()))
                ])),
                addrspec: AddrSpec {
                    local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(b"mary"[..].into()))]),
                    domain: Domain::Atoms(vec![b"x"[..].into(), b"test"[..].into()]),
                }
            }
        );

        mailbox_roundtrip_as(
            r#"jdoe@example.org"#.as_bytes(),
            MailboxRef {
                name: None,
                addrspec: AddrSpec {
                    local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(b"jdoe"[..].into()))]),
                    domain: Domain::Atoms(vec![b"example"[..].into(), b"org"[..].into()]),
                }
            }
        );

        mailbox_roundtrip_as(
            r#"Who? <one@y.test>"#.as_bytes(),
            MailboxRef {
                name: Some(Phrase(vec![PhraseToken::Word(Word::Atom(b"Who?"[..].into()))])),
                addrspec: AddrSpec {
                    local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(b"one"[..].into()))]),
                    domain: Domain::Atoms(vec![b"y"[..].into(), b"test"[..].into()]),
                }
            }
        );

        mailbox_parsed_printed(
            r#"<boss@nil.test>"#.as_bytes(),
            MailboxRef {
                name: None,
                addrspec: AddrSpec {
                    local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(b"boss"[..].into()))]),
                    domain: Domain::Atoms(vec![b"nil"[..].into(), b"test"[..].into()]),
                }
            },
            r#"boss@nil.test"#.as_bytes(),
        );

        mailbox_roundtrip_as(
            r#""Giant; \"Big\" Box" <sysservices@example.net>"#.as_bytes(),
            MailboxRef {
                name: Some(Phrase(vec![
                    PhraseToken::Word(Word::Quoted(QuotedString(vec![
                        b"Giant;"[..].into(),
                        vec![ascii::SP].into(),
                        vec![ascii::DQUOTE].into(),
                        b"Big"[..].into(),
                        vec![ascii::DQUOTE].into(),
                        vec![ascii::SP].into(),
                        b"Box"[..].into()
                    ])))])),
                addrspec: AddrSpec {
                    local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(
                        b"sysservices"[..].into()
                    ))]),
                    domain: Domain::Atoms(vec![b"example"[..].into(), b"net"[..].into()]),
                }
            }
        );
    }

    #[test]
    fn test_obs_domain_list() {
        assert_eq!(
            obs_domain_list(
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
                    Some(Domain::Atoms(vec![b"33+4"[..].into(), b"com"[..].into()])),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(Domain::Atoms(vec![b"example"[..].into(), b"com"[..].into()])),
                    Some(Domain::Atoms(vec![b"yep"[..].into(), b"com"[..].into()])),
                    Some(Domain::Atoms(vec![b"a"[..].into()])),
                    Some(Domain::Atoms(vec![b"b"[..].into()])),
                    None,
                    None,
                    Some(Domain::Atoms(vec![b"c"[..].into()])),
                ]
            ))
        );

        assert_eq!(
            obs_domain_list(b",, ,@foo,"),
            Ok((
                &b""[..],
                vec![
                    None,
                    None,
                    None,
                    Some(Domain::Atoms(vec![b"foo"[..].into()])),
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
                    LocalPartToken::Word(Word::Atom(b"a"[..].into())),
                    LocalPartToken::Dot,
                    LocalPartToken::Dot,
                    LocalPartToken::Word(Word::Atom(b"howard"[..].into())),
                ]),
                domain: Domain::Atoms(vec![b"enron"[..].into(), b"com"[..].into()]),
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
                    LocalPartToken::Word(Word::Atom(b"nelson"[..].into())),
                ]),
                domain: Domain::Atoms(vec![b"enron"[..].into(), b"com"[..].into()]),
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
                    LocalPartToken::Word(Word::Atom(b"ecn2760"[..].into())),
                    LocalPartToken::Dot,
                    LocalPartToken::Word(Word::Atom(b"conf"[..].into())),
                    LocalPartToken::Dot,
                ]),
                domain: Domain::Atoms(vec![b"enron"[..].into(), b"com"[..].into()]),
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
                        QuotedString(vec![b"mark_kopinski/intl/acim/americancentury"[..].into(),])
                    ))]),
                    domain: Domain::Atoms(vec![b"americancentury"[..].into(), b"com"[..].into()]),
                }
            },
            b"mark_kopinski/intl/acim/americancentury@americancentury.com",
        );
    }
}
