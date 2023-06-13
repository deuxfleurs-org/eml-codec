use nom::{
    IResult,
    branch::alt,
    bytes::complete::tag,
    character::complete::satisfy,
    combinator::{into,opt},
    multi::many0,
    sequence::{delimited,pair,tuple},
};

use crate::model::{MailboxRef, AddrSpec};
use crate::misc_token::phrase;
use crate::whitespace::{cfws, fws};
use crate::words::dot_atom;
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
fn angle_addr(input: &str) -> IResult<&str, MailboxRef> {
   delimited(
       pair(opt(cfws), tag("<")),
       into(addr_spec),
       pair(tag(">"), opt(cfws)),
    )(input)
}

/// Add-spec
///
/// ```abnf
///    addr-spec       =   local-part "@" domain
/// ```
pub fn addr_spec(input: &str) -> IResult<&str, AddrSpec> {
    let (input, (local, _, domain)) = tuple((local_part, tag("@"), domain_part))(input)?;
    Ok((input, AddrSpec {
        local_part: local,
        domain: domain,
    }))
}

/// Local part
///
/// ```abnf
///    local-part      =   dot-atom / quoted-string / obs-local-part
/// ```
fn local_part(input: &str) -> IResult<&str, String> {
    alt((into(dot_atom), quoted_string))(input)
}

/// Domain
///
/// ```abnf
///    domain          =   dot-atom / domain-literal / obs-domain
/// ```
fn domain_part(input: &str) -> IResult<&str, String> {
    alt((into(dot_atom), domain_litteral))(input)
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

/// Is domain text
///
/// ```abnf
///   dtext           =   %d33-90 /          ; Printable US-ASCII
///                       %d94-126 /         ;  characters not including
///                       obs-dtext          ;  "[", "]", or "\"
/// ```
pub fn is_dtext(c: char) -> bool {
    (c >= '\x21' && c <= '\x5A') || (c >= '\x5E' && c <= '\x7E') || !c.is_ascii()
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
}
