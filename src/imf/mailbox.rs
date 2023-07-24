use nom::{
    branch::alt,
    bytes::complete::{tag, take_while1},
    combinator::{into, map, opt},
    multi::{many0, separated_list1},
    sequence::{delimited, pair, preceded, terminated, tuple},
    IResult,
};
use std::fmt;

use crate::text::ascii;
use crate::text::misc_token::{phrase, word, Phrase, Word};
use crate::text::whitespace::{cfws, fws, is_obs_no_ws_ctl};
use crate::text::words::atom;

#[derive(PartialEq)]
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

#[derive(Debug, PartialEq)]
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
pub type MailboxList<'a> = Vec<MailboxRef<'a>>;

/// Mailbox
///
/// ```abnf
///    mailbox         =   name-addr / addr-spec
/// ```
pub fn mailbox(input: &[u8]) -> IResult<&[u8], MailboxRef> {
    alt((name_addr, into(addr_spec)))(input)
}

/// Name of the email address
///
/// ```abnf
///    name-addr       =   [display-name] angle-addr
/// ```
fn name_addr(input: &[u8]) -> IResult<&[u8], MailboxRef> {
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
pub fn angle_addr(input: &[u8]) -> IResult<&[u8], AddrSpec> {
    delimited(
        tuple((opt(cfws), tag(&[ascii::LT]), opt(obs_route))),
        addr_spec,
        pair(tag(&[ascii::GT]), opt(cfws)),
    )(input)
}

///    obs-route       =   obs-domain-list ":"
fn obs_route(input: &[u8]) -> IResult<&[u8], Vec<Option<Domain>>> {
    terminated(obs_domain_list, tag(&[ascii::COL]))(input)
}

/// ```abnf
///    obs-domain-list =   *(CFWS / ",") "@" domain
///                        *("," [CFWS] ["@" domain])
/// ```
fn obs_domain_list(input: &[u8]) -> IResult<&[u8], Vec<Option<Domain>>> {
    preceded(
        many0(cfws),
        separated_list1(
            tag(&[ascii::COMMA]),
            preceded(many0(cfws), opt(preceded(tag(&[ascii::AT]), obs_domain))),
        ),
    )(input)
}

/// AddrSpec
///
/// ```abnf
///    addr-spec       =   local-part "@" domain
/// ```
/// @FIXME: this system does not work to alternate between strict and obsolete
/// so I force obsolete for now...
pub fn addr_spec(input: &[u8]) -> IResult<&[u8], AddrSpec> {
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

#[derive(Debug, PartialEq)]
pub enum LocalPartToken<'a> {
    Dot,
    Word(Word<'a>),
}

#[derive(Debug, PartialEq)]
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
fn obs_local_part(input: &[u8]) -> IResult<&[u8], LocalPart> {
    map(
        many0(alt((
            map(tag(&[ascii::PERIOD]), |_| LocalPartToken::Dot),
            map(word, LocalPartToken::Word),
        ))),
        LocalPart,
    )(input)
}

#[derive(PartialEq)]
pub enum Domain<'a> {
    Atoms(Vec<&'a [u8]>),
    Litteral(Vec<&'a [u8]>),
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
            Domain::Litteral(v) => {
                let inner = v
                    .iter()
                    .map(|v| {
                        encoding_rs::UTF_8
                            .decode_without_bom_handling(v)
                            .0
                            .to_string()
                    })
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

/// Obsolete domain
///
/// Rewritten so that obs_domain is a superset
/// of strict_domain.
///
/// ```abnf
///  obs-domain      =   atom *("." atom) / domain-literal
/// ```
pub fn obs_domain(input: &[u8]) -> IResult<&[u8], Domain> {
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
fn domain_litteral(input: &[u8]) -> IResult<&[u8], Domain> {
    delimited(
        pair(opt(cfws), tag(&[ascii::LEFT_BRACKET])),
        inner_domain_litteral,
        pair(tag(&[ascii::RIGHT_BRACKET]), opt(cfws)),
    )(input)
}

fn inner_domain_litteral(input: &[u8]) -> IResult<&[u8], Domain> {
    map(
        terminated(many0(preceded(opt(fws), take_while1(is_dtext))), opt(fws)),
        Domain::Litteral,
    )(input)
}

fn is_strict_dtext(c: u8) -> bool {
    (0x21..=0x5A).contains(&c) || (0x5E..=0x7E).contains(&c)
}

/// Is domain text
///
/// ```abnf
///   dtext           =   %d33-90 /          ; Printable US-ASCII
///                       %d94-126 /         ;  characters not including
///                       obs-dtext          ;  "[", "]", or "\"
///   obs-dtext       =   obs-NO-WS-CTL / quoted-pair
/// ```
pub fn is_dtext(c: u8) -> bool {
    is_strict_dtext(c) || is_obs_no_ws_ctl(c)
    //@FIXME does not support quoted pair yet while RFC requires it
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::quoted::QuotedString;

    #[test]
    fn test_addr_spec() {
        assert_eq!(
            addr_spec(b"alice@example.com"),
            Ok((
                &b""[..],
                AddrSpec {
                    local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(&b"alice"[..]))]),
                    domain: Domain::Atoms(vec![&b"example"[..], &b"com"[..]]),
                }
            ))
        );

        assert_eq!(
            addr_spec(b"jsmith@[192.168.2.1]").unwrap().1.to_string(),
            "jsmith@[192.168.2.1]".to_string(),
        );
        assert_eq!(
            addr_spec(b"jsmith@[IPv6:2001:db8::1]")
                .unwrap()
                .1
                .to_string(),
            "jsmith@[IPv6:2001:db8::1]".to_string(),
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
        assert_eq!(
            addr_spec(b"user+mailbox/department=shipping@example.com")
                .unwrap()
                .1
                .to_string(),
            "user+mailbox/department=shipping@example.com".to_string(),
        );

        assert_eq!(
            addr_spec(b"!#$%&'*+-/=?^_`.{|}~@example.com")
                .unwrap()
                .1
                .to_string(),
            "!#$%&'*+-/=?^_`.{|}~@example.com".to_string(),
        );

        assert_eq!(
            addr_spec(r#""Abc@def"@example.com"#.as_bytes()),
            Ok((
                &b""[..],
                AddrSpec {
                    local_part: LocalPart(vec![LocalPartToken::Word(Word::Quoted(QuotedString(
                        vec![b"Abc@def"]
                    )))]),
                    domain: Domain::Atoms(vec![&b"example"[..], &b"com"[..]]),
                }
            ))
        );
        assert_eq!(
            addr_spec(r#""Fred\ Bloggs"@example.com"#.as_bytes()),
            Ok((
                &b""[..],
                AddrSpec {
                    local_part: LocalPart(vec![LocalPartToken::Word(Word::Quoted(QuotedString(
                        vec![b"Fred", b" ", b"Bloggs"]
                    )))]),
                    domain: Domain::Atoms(vec![&b"example"[..], &b"com"[..]]),
                }
            ))
        );
        assert_eq!(
            addr_spec(r#""Joe.\\Blow"@example.com"#.as_bytes()),
            Ok((
                &b""[..],
                AddrSpec {
                    local_part: LocalPart(vec![LocalPartToken::Word(Word::Quoted(QuotedString(
                        vec![b"Joe.", &[ascii::BACKSLASH], b"Blow"]
                    )))]),
                    domain: Domain::Atoms(vec![&b"example"[..], &b"com"[..]]),
                }
            ))
        );
    }

    #[test]
    fn test_mailbox() {
        assert_eq!(
            mailbox(r#""Joe Q. Public" <john.q.public@example.com>"#.as_bytes()),
            Ok((
                &b""[..],
                MailboxRef {
                    name: Some(Phrase(vec![Word::Quoted(QuotedString(vec![
                        &b"Joe"[..],
                        &[ascii::SP],
                        &b"Q."[..],
                        &[ascii::SP],
                        &b"Public"[..]
                    ]))])),
                    addrspec: AddrSpec {
                        local_part: LocalPart(vec![
                            LocalPartToken::Word(Word::Atom(&b"john"[..])),
                            LocalPartToken::Dot,
                            LocalPartToken::Word(Word::Atom(&b"q"[..])),
                            LocalPartToken::Dot,
                            LocalPartToken::Word(Word::Atom(&b"public"[..])),
                        ]),
                        domain: Domain::Atoms(vec![&b"example"[..], &b"com"[..]]),
                    }
                }
            ))
        );

        assert_eq!(
            mailbox(r#"Mary Smith <mary@x.test>"#.as_bytes()),
            Ok((
                &b""[..],
                MailboxRef {
                    name: Some(Phrase(vec![
                        Word::Atom(&b"Mary"[..]),
                        Word::Atom(&b"Smith"[..])
                    ])),
                    addrspec: AddrSpec {
                        local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(&b"mary"[..]))]),
                        domain: Domain::Atoms(vec![&b"x"[..], &b"test"[..]]),
                    }
                }
            ))
        );

        assert_eq!(
            mailbox(r#"jdoe@example.org"#.as_bytes()),
            Ok((
                &b""[..],
                MailboxRef {
                    name: None,
                    addrspec: AddrSpec {
                        local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(&b"jdoe"[..]))]),
                        domain: Domain::Atoms(vec![&b"example"[..], &b"org"[..]]),
                    }
                }
            ))
        );

        assert_eq!(
            mailbox(r#"Who? <one@y.test>"#.as_bytes()),
            Ok((
                &b""[..],
                MailboxRef {
                    name: Some(Phrase(vec![Word::Atom(&b"Who?"[..])])),
                    addrspec: AddrSpec {
                        local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(&b"one"[..]))]),
                        domain: Domain::Atoms(vec![&b"y"[..], &b"test"[..]]),
                    }
                }
            ))
        );

        assert_eq!(
            mailbox(r#"<boss@nil.test>"#.as_bytes()),
            Ok((
                &b""[..],
                MailboxRef {
                    name: None,
                    addrspec: AddrSpec {
                        local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(&b"boss"[..]))]),
                        domain: Domain::Atoms(vec![&b"nil"[..], &b"test"[..]]),
                    }
                }
            ))
        );

        assert_eq!(
            mailbox(r#""Giant; \"Big\" Box" <sysservices@example.net>"#.as_bytes()),
            Ok((
                &b""[..],
                MailboxRef {
                    name: Some(Phrase(vec![Word::Quoted(QuotedString(vec![
                        &b"Giant;"[..],
                        &[ascii::SP],
                        &[ascii::DQUOTE],
                        &b"Big"[..],
                        &[ascii::DQUOTE],
                        &[ascii::SP],
                        &b"Box"[..]
                    ]))])),
                    addrspec: AddrSpec {
                        local_part: LocalPart(vec![LocalPartToken::Word(Word::Atom(
                            &b"sysservices"[..]
                        ))]),
                        domain: Domain::Atoms(vec![&b"example"[..], &b"net"[..]]),
                    }
                }
            ))
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
                    Some(Domain::Atoms(vec![&b"33+4"[..], &b"com"[..]])),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(Domain::Atoms(vec![&b"example"[..], &b"com"[..]])),
                    Some(Domain::Atoms(vec![&b"yep"[..], &b"com"[..]])),
                    Some(Domain::Atoms(vec![&b"a"[..]])),
                    Some(Domain::Atoms(vec![&b"b"[..]])),
                    None,
                    None,
                    Some(Domain::Atoms(vec![&b"c"[..]])),
                ]
            ))
        );
    }

    #[test]
    fn test_enron1() {
        assert_eq!(
            addr_spec("a..howard@enron.com".as_bytes()),
            Ok((
                &b""[..],
                AddrSpec {
                    local_part: LocalPart(vec![
                        LocalPartToken::Word(Word::Atom(&b"a"[..])),
                        LocalPartToken::Dot,
                        LocalPartToken::Dot,
                        LocalPartToken::Word(Word::Atom(&b"howard"[..])),
                    ]),
                    domain: Domain::Atoms(vec![&b"enron"[..], &b"com"[..]]),
                }
            ))
        );
    }

    #[test]
    fn test_enron2() {
        assert_eq!(
            addr_spec(".nelson@enron.com".as_bytes()),
            Ok((
                &b""[..],
                AddrSpec {
                    local_part: LocalPart(vec![
                        LocalPartToken::Dot,
                        LocalPartToken::Word(Word::Atom(&b"nelson"[..])),
                    ]),
                    domain: Domain::Atoms(vec![&b"enron"[..], &b"com"[..]]),
                }
            ))
        );
    }

    #[test]
    fn test_enron3() {
        assert_eq!(
            addr_spec("ecn2760.conf.@enron.com".as_bytes()),
            Ok((
                &b""[..],
                AddrSpec {
                    local_part: LocalPart(vec![
                        LocalPartToken::Word(Word::Atom(&b"ecn2760"[..])),
                        LocalPartToken::Dot,
                        LocalPartToken::Word(Word::Atom(&b"conf"[..])),
                        LocalPartToken::Dot,
                    ]),
                    domain: Domain::Atoms(vec![&b"enron"[..], &b"com"[..]]),
                }
            ))
        );
    }

    #[test]
    fn test_enron4() {
        assert_eq!(
            mailbox(
                r#"<"mark_kopinski/intl/acim/americancentury"@americancentury.com@enron.com>"#
                    .as_bytes()
            ),
            Ok((
                &b""[..],
                MailboxRef {
                    name: None,
                    addrspec: AddrSpec {
                        local_part: LocalPart(vec![LocalPartToken::Word(Word::Quoted(
                            QuotedString(vec![&b"mark_kopinski/intl/acim/americancentury"[..],])
                        ))]),
                        domain: Domain::Atoms(vec![&b"americancentury"[..], &b"com"[..]]),
                    }
                }
            ))
        );
    }
}
