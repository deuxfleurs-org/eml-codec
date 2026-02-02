use bounded_static::ToStatic;
use nom::{
    branch::alt,
    bytes::complete::{tag, take, take_while1},
    combinator::opt,
    multi::many0,
    sequence::{pair, preceded},
    IResult,
};
use std::borrow::Cow;

use crate::print::Formatter;
use crate::text::ascii;
use crate::text::whitespace::{cfws, fws, is_obs_no_ws_ctl};
use crate::text::words::is_vchar;

#[derive(Debug, PartialEq, Default, Clone, ToStatic)]
pub struct QuotedString<'a>(pub Vec<Cow<'a, [u8]>>);

impl<'a> QuotedString<'a> {
    pub fn push(&mut self, e: &'a [u8]) {
        self.0.push(Cow::Borrowed(e))
    }

    // XXX remove?
    pub fn to_string(&self) -> String {
        let enc = encoding_rs::UTF_8;
        let size = self.0.iter().fold(0, |acc, v| acc + v.len());

        self.0
            .iter()
            .fold(String::with_capacity(size), |mut acc, v| {
                let (content, _) = enc.decode_without_bom_handling(v);
                acc.push_str(content.as_ref());
                acc
            })
    }

    pub fn bytes<'b>(&'b self) -> QuotedStringBytes<'a, 'b> {
        QuotedStringBytes { q: self, outer: 0, inner: 0 }
    }
}

#[derive(Clone)]
pub struct QuotedStringBytes<'a, 'b> {
    q: &'b QuotedString<'a>,
    outer: usize,
    inner: usize,
}

impl<'a, 'b> Iterator for QuotedStringBytes<'a, 'b> {
    type Item = u8;
    fn next(&mut self) -> Option<Self::Item> {
        if self.outer < self.q.0.len() {
            if self.inner < self.q.0[self.outer].len() {
                let b = self.q.0[self.outer][self.inner];
                self.inner += 1;
                return Some(b)
            } else {
                self.outer += 1;
                self.inner = 0;
                return self.next()
            }
        } else {
            return None
        }
    }
}

/// Quoted pair
///
/// ```abnf
///    quoted-pair     =   ("\" (VCHAR / WSP)) / obs-qp
///    obs-qp          =   "\" (%d0 / obs-NO-WS-CTL / LF / CR)
/// ```
/// We parse quoted pairs even more liberally, allowing any byte after
/// the backslash.
pub fn quoted_pair(input: &[u8]) -> IResult<&[u8], &[u8]> {
    preceded(tag(&[ascii::BACKSLASH]), take(1usize))(input)
}

/// Allowed characters in quote
///
/// ```abnf
///   qtext           =   %d33 /             ; Printable US-ASCII
///                       %d35-91 /          ;  characters not including
///                       %d93-126 /         ;  "\" or the quote character
///                       obs-qtext
/// ```
fn is_restr_qtext(c: u8) -> bool {
    c == ascii::EXCLAMATION
        || (ascii::NUM..=ascii::LEFT_BRACKET).contains(&c)
        || (ascii::RIGHT_BRACKET..=ascii::TILDE).contains(&c)
}

fn is_qtext(c: u8) -> bool {
    is_restr_qtext(c) || is_obs_no_ws_ctl(c)
}

/// Quoted pair content
///
/// ```abnf
///   qcontent        =   qtext / quoted-pair
/// ```
fn qcontent(input: &[u8]) -> IResult<&[u8], &[u8]> {
    alt((take_while1(is_qtext), quoted_pair))(input)
}

/// Quoted string
///
/// ```abnf
/// quoted-string   =   [CFWS]
///                     DQUOTE *([FWS] qcontent) [FWS] DQUOTE
///                     [CFWS]
/// ```
pub fn quoted_string(input: &[u8]) -> IResult<&[u8], QuotedString<'_>> {
    let (input, _) = opt(cfws)(input)?;
    let (input, _) = tag("\"")(input)?;
    let (input, content) = many0(pair(opt(fws), qcontent))(input)?;

    // Rebuild string
    let mut qstring = content
        .iter()
        .fold(QuotedString::default(), |mut acc, (maybe_wsp, c)| {
            if maybe_wsp.is_some() {
                acc.push(&[ascii::SP]);
            }
            acc.push(c);
            acc
        });

    let (input, maybe_wsp) = opt(fws)(input)?;
    if maybe_wsp.is_some() {
        qstring.push(&[ascii::SP]);
    }

    let (input, _) = tag("\"")(input)?;
    let (input, _) = opt(cfws)(input)?;
    Ok((input, qstring))
}

pub fn print_quoted<I>(fmt: &mut impl Formatter, data: I) -> std::io::Result<()>
where
    I: IntoIterator<Item = u8>
{
    fmt.write_bytes(b"\"")?;
    for b in data.into_iter() {
        if is_restr_qtext(b) {
            fmt.write_bytes(&[b])?;
        } else if ascii::WS.contains(&b) {
            // NOTE: we can either output the whitespace as folding
            // whitespace or to escape it; we choose to output it as folding
            // whitespace which helps performing line folding.
            fmt.write_fws_bytes(&[b])?;
        } else if is_vchar(b) {
            fmt.write_bytes(&[b'\\', b])?;
        } else {
            // RFC5322 does not allow escaping bytes other than VCHAR in
            // quoted strings. We drop them.
            // TODO: return the invalid input bytes that were skipped.
            ()
        }
    }
    fmt.write_bytes(b"\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::print::with_line_folder;

    #[test]
    fn test_quoted_string_parser() {
        assert_eq!(
            quoted_string(b" \"hello\\\"world\" ").unwrap().1,
            QuotedString(vec![
                b"hello".into(),
                vec![ascii::DQUOTE].into(),
                b"world".into(),
            ])
        );

        assert_eq!(
            quoted_string(b"\"hello\r\n world\""),
            Ok((
                &b""[..],
                QuotedString(vec![
                    b"hello".into(),
                    vec![ascii::SP].into(),
                    b"world".into(),
                ])
            )),
        );
    }

    #[test]
    fn test_quoted_string_printer() {
        let out = with_line_folder(|f| {
            print_quoted(
                f,
                QuotedString(vec![
                    b"hello".into(),
                    vec![ascii::DQUOTE].into(),
                    b" world".into(),
                ]).bytes()
            ).unwrap();
        });
        assert_eq!(out, b"\"hello\\\" world\"");
    }

    use crate::text::ascii;

    #[test]
    fn test_quoted_string_object() {
        assert_eq!(
            QuotedString(vec![
                b"hello".into(),
                vec![ascii::SP].into(),
                b"world".into(),
            ])
            .to_string(),
            "hello world".to_string(),
        );
    }
}
