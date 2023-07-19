use nom::{
    branch::alt,
    bytes::complete::{tag, take_while1},
    combinator::{into, map, opt},
    multi::{many0, separated_list1},
    sequence::{delimited, pair, preceded, terminated, tuple},
    IResult,
};

use crate::text::misc_token::{phrase, word, Word, Phrase};
use crate::text::whitespace::{cfws, fws, is_obs_no_ws_ctl};
use crate::text::words::{atom};
use crate::text::ascii;

#[derive(Debug, PartialEq)]
pub struct AddrSpec<'a> {
    pub local_part: LocalPart<'a>,
    pub domain: Domain<'a>,
}
impl<'a> AddrSpec<'a> {
    pub fn to_string(&self) -> String {
        format!("{}@{}", self.local_part.to_string(), self.domain.to_string())
    }
}

#[derive(Debug, PartialEq)]
pub struct MailboxRef<'a> {
    // The actual "email address" like hello@example.com
    pub addrspec: AddrSpec<'a>,
    pub name: Option<Phrase<'a>>,
}
impl<'a> MailboxRef<'a> {
    pub fn to_string(&self) -> String {
        match &self.name {
            Some(n) => format!("{} <{}>", n.to_string(), self.addrspec.to_string()),
            None => self.addrspec.to_string()
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
    let (input, mut mbox) = angle_addr(input)?;
    mbox.name = name;
    Ok((input, mbox))
}

/// Enclosed addr-spec with < and >
///
/// ```abnf
/// angle-addr      =   [CFWS] "<" addr-spec ">" [CFWS] /
///                     obs-angle-addr
/// ```
pub fn angle_addr(input: &[u8]) -> IResult<&[u8], MailboxRef> {
    delimited(
        tuple((opt(cfws), tag(&[ascii::LT]), opt(obs_route))),
        into(addr_spec),
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
    ))(input)
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
            many0(pair(tag(&[ascii::AT]), obs_domain)), // for compatibility reasons
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
        self.0.iter().fold(
            String::new(),
            |mut acc, token| {
                match token {
                    LocalPartToken::Dot => acc.push('.'),
                    LocalPartToken::Word(v) => acc.push_str(v.to_string().as_ref()),
                }
                acc
            }
        )
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
            map(word, |v| LocalPartToken::Word(v)),
        ))),
        |v| LocalPart(v),
    )(input)
}

#[derive(Debug, PartialEq)]
pub enum Domain<'a> {
    Atoms(Vec<&'a [u8]>),
    Litteral(Vec<&'a [u8]>),
}

impl<'a> Domain<'a> {
    pub fn to_string(&self) -> String {
        match self {
            Domain::Atoms(v) => v.iter().map(|v| encoding_rs::UTF_8.decode_without_bom_handling(v).0.to_string()).collect::<Vec<String>>().join("."),
            Domain::Litteral(v) => v.iter().map(|v| encoding_rs::UTF_8.decode_without_bom_handling(v).0.to_string()).collect::<Vec<String>>().join(" "),
        } 
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
        map(separated_list1(tag("."), atom), |v| Domain::Atoms(v)),
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
            |v| Domain::Litteral(v),
        )(input)
}

fn is_strict_dtext(c: u8) -> bool {
    (c >= 0x21 && c <= 0x5A) || (c >= 0x5E && c <= 0x7E)
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

    #[test]
    fn test_addr_spec() {
        assert_eq!(
            addr_spec("alice@example.com"),
            Ok((
                "",
                AddrSpec {
                    local_part: "alice".into(),
                    domain: "example.com".into()
                }
            ))
        );

        assert_eq!(
            addr_spec("jsmith@[192.168.2.1]"),
            Ok((
                "",
                AddrSpec {
                    local_part: "jsmith".into(),
                    domain: "192.168.2.1".into()
                }
            ))
        );
        assert_eq!(
            addr_spec("jsmith@[IPv6:2001:db8::1]"),
            Ok((
                "",
                AddrSpec {
                    local_part: "jsmith".into(),
                    domain: "IPv6:2001:db8::1".into()
                }
            ))
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
            addr_spec("user+mailbox/department=shipping@example.com"),
            Ok((
                "",
                AddrSpec {
                    local_part: "user+mailbox/department=shipping".into(),
                    domain: "example.com".into()
                }
            ))
        );
        assert_eq!(
            addr_spec("!#$%&'*+-/=?^_`.{|}~@example.com"),
            Ok((
                "",
                AddrSpec {
                    local_part: "!#$%&'*+-/=?^_`.{|}~".into(),
                    domain: "example.com".into()
                }
            ))
        );
        assert_eq!(
            addr_spec(r#""Abc@def"@example.com"#),
            Ok((
                "",
                AddrSpec {
                    local_part: "Abc@def".into(),
                    domain: "example.com".into()
                }
            ))
        );
        assert_eq!(
            addr_spec(r#""Fred\ Bloggs"@example.com"#),
            Ok((
                "",
                AddrSpec {
                    local_part: "Fred Bloggs".into(),
                    domain: "example.com".into()
                }
            ))
        );
        assert_eq!(
            addr_spec(r#""Joe.\\Blow"@example.com"#),
            Ok((
                "",
                AddrSpec {
                    local_part: r#"Joe.\Blow"#.into(),
                    domain: "example.com".into()
                }
            ))
        );
    }

    #[test]
    fn test_mailbox() {
        assert_eq!(
            mailbox(r#""Joe Q. Public" <john.q.public@example.com>"#),
            Ok((
                "",
                MailboxRef {
                    name: Some("Joe Q. Public".into()),
                    addrspec: AddrSpec {
                        local_part: "john.q.public".into(),
                        domain: "example.com".into(),
                    }
                }
            ))
        );

        assert_eq!(
            mailbox(r#"Mary Smith <mary@x.test>"#),
            Ok((
                "",
                MailboxRef {
                    name: Some("Mary Smith".into()),
                    addrspec: AddrSpec {
                        local_part: "mary".into(),
                        domain: "x.test".into(),
                    }
                }
            ))
        );

        assert_eq!(
            mailbox(r#"jdoe@example.org"#),
            Ok((
                "",
                MailboxRef {
                    name: None,
                    addrspec: AddrSpec {
                        local_part: "jdoe".into(),
                        domain: "example.org".into(),
                    }
                }
            ))
        );

        assert_eq!(
            mailbox(r#"Who? <one@y.test>"#),
            Ok((
                "",
                MailboxRef {
                    name: Some("Who?".into()),
                    addrspec: AddrSpec {
                        local_part: "one".into(),
                        domain: "y.test".into(),
                    }
                }
            ))
        );

        assert_eq!(
            mailbox(r#"<boss@nil.test>"#),
            Ok((
                "",
                MailboxRef {
                    name: None,
                    addrspec: AddrSpec {
                        local_part: "boss".into(),
                        domain: "nil.test".into(),
                    }
                }
            ))
        );

        assert_eq!(
            mailbox(r#""Giant; \"Big\" Box" <sysservices@example.net>"#),
            Ok((
                "",
                MailboxRef {
                    name: Some(r#"Giant; "Big" Box"#.into()),
                    addrspec: AddrSpec {
                        local_part: "sysservices".into(),
                        domain: "example.net".into(),
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
            ),
            Ok((
                "",
                vec![
                    "33+4.com".into(),
                    "example.com".into(),
                    "yep.com".into(),
                    "a".into(),
                    "b".into(),
                    "c".into()
                ]
            ))
        );
    }

    #[test]
    fn test_enron1() {
        assert_eq!(
            addr_spec("a..howard@enron.com"),
            Ok((
                "",
                AddrSpec {
                    local_part: "a..howard".into(),
                    domain: "enron.com".into(),
                }
            ))
        );
    }

    #[test]
    fn test_enron2() {
        assert_eq!(
            addr_spec(".nelson@enron.com"),
            Ok((
                "",
                AddrSpec {
                    local_part: ".nelson".into(),
                    domain: "enron.com".into(),
                }
            ))
        );
    }

    #[test]
    fn test_enron3() {
        assert_eq!(
            addr_spec("ecn2760.conf.@enron.com"),
            Ok((
                "",
                AddrSpec {
                    local_part: "ecn2760.conf.".into(),
                    domain: "enron.com".into(),
                }
            ))
        );
    }

    #[test]
    fn test_enron4() {
        assert_eq!(
            mailbox(r#"<"mark_kopinski/intl/acim/americancentury"@americancentury.com@enron.com>"#),
            Ok((
                "",
                MailboxRef {
                    name: None,
                    addrspec: AddrSpec {
                        local_part: "mark_kopinski/intl/acim/americancentury".into(),
                        domain: "americancentury.com".into(),
                    }
                }
            ))
        );
    }
}
