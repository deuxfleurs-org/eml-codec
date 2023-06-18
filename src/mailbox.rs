use std::borrow::Cow;
use nom::{
    IResult,
    Parser,
    branch::alt,
    bytes::complete::{tag, is_a},
    character::complete::satisfy,
    combinator::{into,map,opt,recognize},
    multi::{separated_list1, fold_many0, many0, many1},
    sequence::{delimited,pair,preceded,terminated,tuple},
};

use crate::model::{MailboxRef, AddrSpec};
use crate::misc_token::{phrase, word};
use crate::whitespace::{cfws, fws, is_obs_no_ws_ctl};
use crate::words::{atom, dot_atom};
use crate::quoted::quoted_string;

/// Mailbox
///
/// ```abnf
///    mailbox         =   name-addr / addr-spec
/// ```
pub fn mailbox(input: &str) -> IResult<&str, MailboxRef> {
    alt((name_addr, into(addr_spec)))(input)
}

/// Name of the email address
///
/// ```abnf
///    name-addr       =   [display-name] angle-addr
/// ```
fn name_addr(input: &str) -> IResult<&str, MailboxRef> {
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
pub fn angle_addr(input: &str) -> IResult<&str, MailboxRef> {
   delimited(
       tuple((opt(cfws), tag("<"), opt(obs_route))),
       into(addr_spec),
       pair(tag(">"), opt(cfws)),
    )(input)
}

///    obs-route       =   obs-domain-list ":"
fn obs_route(input: &str) -> IResult<&str, Vec<String>> {
    terminated(obs_domain_list, tag(":"))(input)
}

/// ```abnf
///    obs-domain-list =   *(CFWS / ",") "@" domain
///                        *("," [CFWS] ["@" domain])
/// ```
fn obs_domain_list(input: &str) -> IResult<&str, Vec<String>> {
    //@FIXME complexity is O(n) in term of domains here.
    let (input, head) = preceded(pair(many0(alt((recognize(cfws), tag(",")))), tag("@")), obs_domain)(input)?; 
    let (input, mut rest) = obs_domain_list_rest(input)?;
    rest.insert(0, head);
    Ok(("", rest))
}

fn obs_domain_list_rest(input: &str) -> IResult<&str, Vec<String>> {
    map(
        many0(preceded(
            pair(tag(","), opt(cfws)),
            opt(preceded(tag("@"), obs_domain)),
        )),
        |v: Vec<Option<String>>| v.into_iter().flatten().collect()
    )(input)
}

/// AddrSpec
///
/// ```abnf
///    addr-spec       =   local-part "@" domain
/// ```
/// @FIXME: this system does not work to alternate between strict and obsolete
/// so I force obsolete for now...
pub fn addr_spec(input: &str) -> IResult<&str, AddrSpec> {
    map(
        tuple((obs_local_part, tag("@"), obs_domain, many0(pair(tag("@"), obs_domain)))),
        |(local_part, _, domain, _)| 
            AddrSpec { local_part, domain },
    )(input)
}

/// Local part
///
/// ```abnf
///    local-part      =   dot-atom / quoted-string / obs-local-part
/// ```
fn strict_local_part(input: &str) -> IResult<&str, String> {
    alt((into(dot_atom), quoted_string))(input)
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
fn obs_local_part(input: &str) -> IResult<&str, String> {
    fold_many0(
        alt((map(is_a("."), Cow::Borrowed), word)),
        String::new,
        |acc, chunk| acc + &chunk)(input)
}

/// Domain
///
/// ```abnf
///    domain          =   dot-atom / domain-literal
/// ```
pub fn strict_domain(input: &str) -> IResult<&str, String> {
    alt((into(dot_atom), domain_litteral))(input)
}

/// Obsolete domain
///
/// Rewritten so that obs_domain is a superset
/// of strict_domain.
///
/// ```abnf
///  obs-domain      =   atom *("." atom) / domain-literal
/// ```
pub fn obs_domain(input: &str) -> IResult<&str, String> {
    alt((map(separated_list1(tag("."), atom), |v| v.join(".")), domain_litteral))(input)
}

/// Domain litteral
///
/// ```abnf
///    domain-literal  =   [CFWS] "[" *([FWS] dtext) [FWS] "]" [CFWS]
/// ```
fn domain_litteral(input: &str) -> IResult<&str, String> {
    delimited(
        pair(opt(cfws), tag("[")),
        inner_domain_litteral,
        pair(tag("]"), opt(cfws))
    )(input)
}

fn inner_domain_litteral(input: &str) -> IResult<&str, String> {
    let (input, (cvec, maybe_wsp)) = pair(many0(pair(opt(fws), satisfy(is_dtext))), opt(fws))(input)?;
    let mut domain = cvec.iter().fold(
        String::with_capacity(16),
        |mut acc, (maybe_wsp, c)| {
            if let Some(wsp) = maybe_wsp {
                acc.push(*wsp);
            }
            acc.push(*c);
            acc
        });
    if let Some(wsp) = maybe_wsp {
        domain.push(wsp);
    }

    Ok((input, domain))
}


fn is_strict_dtext(c: char) -> bool {
    (c >= '\x21' && c <= '\x5A') || (c >= '\x5E' && c <= '\x7E') || !c.is_ascii()
}

/// Is domain text
///
/// ```abnf
///   dtext           =   %d33-90 /          ; Printable US-ASCII
///                       %d94-126 /         ;  characters not including
///                       obs-dtext          ;  "[", "]", or "\"
///   obs-dtext       =   obs-NO-WS-CTL / quoted-pair
/// ```
pub fn is_dtext(c: char) -> bool {
    is_strict_dtext(c) || is_obs_no_ws_ctl(c) 
    //@FIXME does not support quoted pair yet while RFC requires it
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_addr_spec() {
        assert_eq!(addr_spec("alice@example.com"), Ok(("", AddrSpec{local_part: "alice".into(), domain: "example.com".into() })));

        assert_eq!(addr_spec("jsmith@[192.168.2.1]"), Ok(("", AddrSpec{local_part: "jsmith".into(), domain: "192.168.2.1".into() })));
        assert_eq!(addr_spec("jsmith@[IPv6:2001:db8::1]"), Ok(("", AddrSpec{local_part: "jsmith".into(), domain: "IPv6:2001:db8::1".into() })));

        // UTF-8
        assert_eq!(addr_spec("用户@例子.广告"), Ok(("", AddrSpec{local_part: "用户".into(), domain: "例子.广告".into()})));

        // ASCII Edge cases
        assert_eq!(
            addr_spec("user+mailbox/department=shipping@example.com"),
            Ok(("", AddrSpec{local_part: "user+mailbox/department=shipping".into(), domain: "example.com".into()})));
        assert_eq!(
            addr_spec("!#$%&'*+-/=?^_`.{|}~@example.com"),
            Ok(("", AddrSpec{local_part: "!#$%&'*+-/=?^_`.{|}~".into(), domain: "example.com".into()})));
        assert_eq!(
            addr_spec(r#""Abc@def"@example.com"#),
            Ok(("", AddrSpec{local_part: "Abc@def".into(), domain: "example.com".into()})));
        assert_eq!(addr_spec(r#""Fred\ Bloggs"@example.com"#), Ok(("", AddrSpec{local_part: "Fred Bloggs".into(), domain: "example.com".into()})));
        assert_eq!(addr_spec(r#""Joe.\\Blow"@example.com"#), Ok(("", AddrSpec{local_part: r#"Joe.\Blow"#.into(), domain: "example.com".into()})));
    }

    #[test]
    fn test_mailbox() {
        assert_eq!(mailbox(r#""Joe Q. Public" <john.q.public@example.com>"#), Ok(("", MailboxRef {
            name: Some("Joe Q. Public".into()),
            addrspec: AddrSpec {
                local_part: "john.q.public".into(),
                domain: "example.com".into(),
            }
        })));

        assert_eq!(mailbox(r#"Mary Smith <mary@x.test>"#), Ok(("", MailboxRef {
            name: Some("Mary Smith".into()),
            addrspec: AddrSpec {
                local_part: "mary".into(),
                domain: "x.test".into(),
            }
        })));

        assert_eq!(mailbox(r#"jdoe@example.org"#), Ok(("", MailboxRef {
            name: None,
            addrspec: AddrSpec {
                local_part: "jdoe".into(),
                domain: "example.org".into(),
            }
        })));

        assert_eq!(mailbox(r#"Who? <one@y.test>"#), Ok(("", MailboxRef {
            name: Some("Who?".into()),
            addrspec: AddrSpec {
                local_part: "one".into(),
                domain: "y.test".into(),
            }
        })));

        assert_eq!(mailbox(r#"<boss@nil.test>"#), Ok(("", MailboxRef {
            name: None,
            addrspec: AddrSpec {
                local_part: "boss".into(),
                domain: "nil.test".into(),
            }
        })));

        assert_eq!(mailbox(r#""Giant; \"Big\" Box" <sysservices@example.net>"#), Ok(("", MailboxRef {
            name: Some(r#"Giant; "Big" Box"#.into()),
            addrspec: AddrSpec {
                local_part: "sysservices".into(),
                domain: "example.net".into(),
            }
        })));
    }

    #[test]
    fn test_obs_domain_list() {
        assert_eq!(obs_domain_list(r#"(shhh it's coming)
 ,
 (not yet)
 @33+4.com,,,,
 ,,,,
 (again)
 @example.com,@yep.com,@a,@b,,,@c"#),
            Ok(("", vec!["33+4.com".into(), "example.com".into(), "yep.com".into(), "a".into(), "b".into(), "c".into()]))
        );
    }

    #[test]
    fn test_enron1() {
        assert_eq!(
            addr_spec("a..howard@enron.com"),
            Ok(("", AddrSpec {
                local_part: "a..howard".into(),
                domain: "enron.com".into(),
            }))
        );
    }

    #[test]
    fn test_enron2() {
        assert_eq!(
            addr_spec(".nelson@enron.com"),
            Ok(("", AddrSpec {
                local_part: ".nelson".into(),
                domain: "enron.com".into(),
            }))
        );
    }

    #[test]
    fn test_enron3() {
        assert_eq!(
            addr_spec("ecn2760.conf.@enron.com"),
            Ok(("", AddrSpec {
                local_part: "ecn2760.conf.".into(),
                domain: "enron.com".into(),
            }))
        );
    }


    #[test]
    fn test_enron4() {
        assert_eq!(
            mailbox(r#"<"mark_kopinski/intl/acim/americancentury"@americancentury.com@enron.com>"#),
            Ok(("", MailboxRef {
                name: None,
                addrspec: AddrSpec {
                    local_part: "mark_kopinski/intl/acim/americancentury".into(),
                    domain: "americancentury.com".into(),
                }
            }))
        );
    }
}
