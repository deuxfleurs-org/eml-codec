#[cfg(feature = "arbitrary")]
use arbitrary::Arbitrary;
#[cfg(feature = "arbitrary")]
use std::ops::ControlFlow;
use bounded_static::ToStatic;
use nom::{
    branch::alt,
    bytes::complete::{tag, take, take_while1},
    combinator::{map, opt},
    multi::many0,
    sequence::{pair, preceded},
    IResult,
};
use std::borrow::Cow;
use std::fmt;

#[cfg(feature = "arbitrary")]
use crate::{
    arbitrary_utils::arbitrary_vec_where,
    fuzz_eq::FuzzEq,
};
use crate::print::Formatter;
use crate::text::ascii;
use crate::text::whitespace::{cfws, fws, is_obs_no_ws_ctl};
use crate::text::words::is_vchar;

// A quoted string contains bytes that satisfy `is_vchar` or are in `ascii::WS`.
#[derive(PartialEq, Default, Clone, ToStatic)]
pub struct QuotedString<'a>(pub Vec<Cow<'a, [u8]>>);

impl<'a> fmt::Debug for QuotedString<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_tuple("QuotedString")
            .field(&self.0.iter()
                   .map(|b| String::from_utf8_lossy(&b))
                   .collect::<Vec<_>>())
            .finish()
    }
}

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

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for QuotedString<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<QuotedString<'a>> {
        let mut chunks = Vec::new();
        u.arbitrary_loop(None, Some(10), |u| {
            let bytes = arbitrary_vec_where(u, |b| {
                is_vchar(b) || ascii::WS.contains(&b)
            })?;
            chunks.push(Cow::Owned(bytes));
            Ok(ControlFlow::Continue(()))
        })?;
        Ok(QuotedString(chunks))
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> FuzzEq for QuotedString<'a> {
    fn fuzz_eq(&self, other: &Self) -> bool {
        self.bytes().collect::<Vec<_>>() == other.bytes().collect::<Vec<_>>()
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
///
/// However, we only return `Some(_)` for quoted pairs that are valid
/// according to the strict syntax; other quoted pairs cannot be printed
/// back and wee chose to ignore them.
pub fn quoted_pair(input: &[u8]) -> IResult<&[u8], Option<&[u8]>> {
    preceded(
        tag(&[ascii::BACKSLASH]),
        map(take(1usize), |b: &[u8]| {
            is_strict_quoted_pair(b[0]).then_some(b)
        })
    )(input)
}

fn is_strict_quoted_pair(b: u8) -> bool {
    is_vchar(b) || ascii::WS.contains(&b)
}

/// Allowed characters in quote
///
/// ```abnf
///   qtext           =   %d33 /             ; Printable US-ASCII
///                       %d35-91 /          ;  characters not including
///                       %d93-126 /         ;  "\" or the quote character
///                       obs-qtext
/// ```
fn is_strict_qtext(c: u8) -> bool {
    c == ascii::EXCLAMATION
        || (ascii::NUM..=ascii::LEFT_BRACKET).contains(&c)
        || (ascii::RIGHT_BRACKET..=ascii::TILDE).contains(&c)
}

fn is_obs_qtext(c: u8) -> bool {
    is_obs_no_ws_ctl(c)
}

/// Quoted pair content
///
/// ```abnf
///   qcontent        =   qtext / quoted-pair
/// ```
///
/// Like for `quoted_pair`, this supports the obsolete syntax but
/// returns `None` in this case.
fn qcontent(input: &[u8]) -> IResult<&[u8], Option<&[u8]>> {
    alt((
        map(take_while1(is_strict_qtext), Some),
        map(take_while1(is_obs_qtext), |_| None),
        quoted_pair,
    ))(input)
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
    let (input, maybe_wsp) = opt(fws)(input)?;
    let (input, _) = tag("\"")(input)?;
    let (input, _) = opt(cfws)(input)?;

    // Rebuild string
    let mut qstring = content
        .iter()
        .fold(QuotedString::default(), |mut acc, (maybe_wsp, c)| {
            for wsp in maybe_wsp.into_iter().flat_map(|v| v.into_iter()) {
                acc.push(wsp);
            }
            if let Some(c) = c {
                acc.push(c);
            }
            acc
        });

    for wsp in maybe_wsp.into_iter().flat_map(|v| v.into_iter()) {
        qstring.push(wsp);
    }

    Ok((input, qstring))
}

pub fn print_quoted<I>(fmt: &mut impl Formatter, data: I)
where
    I: IntoIterator<Item = u8>
{
    fmt.write_bytes(b"\"");
    for b in data.into_iter() {
        if is_strict_qtext(b) {
            fmt.write_bytes(&[b]);
        } else if ascii::WS.contains(&b) {
            // NOTE: we can either output the whitespace as folding
            // whitespace or to escape it; we choose to output it as folding
            // whitespace which helps performing line folding.
            fmt.write_fws_bytes(&[b]);
        } else if is_vchar(b) {
            fmt.write_bytes(&[b'\\', b]);
        } else {
            // RFC5322 does not allow escaping bytes other than VCHAR in
            // quoted strings. We drop them.
            // NOTE: this case shouldn't happen in practice, because
            // non-displayable quoted pairs are already dropped during
            // parsing...
            // TODO: return the invalid input bytes that were skipped.
            ()
        }
    }
    fmt.write_bytes(b"\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::print::tests::with_formatter;

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

        assert_eq!(
            quoted_string(b"\"\t\""),
            Ok((
                &b""[..],
                QuotedString(vec![
                    b"\t".into(),
                ])
            )),
        );
    }

    #[test]
    fn test_quoted_string_printer() {
        let out = with_formatter(|f| {
            print_quoted(
                f,
                QuotedString(vec![
                    b"hello".into(),
                    vec![ascii::DQUOTE].into(),
                    b" world".into(),
                ]).bytes()
            );
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
