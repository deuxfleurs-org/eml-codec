#[cfg(feature = "arbitrary")]
use arbitrary::Arbitrary;
#[cfg(feature = "arbitrary")]
use std::ops::ControlFlow;
use bounded_static::ToStatic;
use nom::{
    branch::alt,
    bytes::complete::{tag, take, take_while1},
    combinator::{map, opt, verify},
    multi::many0,
    sequence::{pair, preceded},
    IResult,
};
use std::borrow::Cow;
use std::fmt;

#[cfg(feature = "arbitrary")]
use crate::{
    arbitrary_utils::arbitrary_string_where,
    fuzz_eq::FuzzEq,
};
use crate::print::{Print, Formatter, ToStringFromPrint};
use crate::text::ascii;
use crate::text::whitespace::{cfws, fws, is_obs_no_ws_ctl};
use crate::text::words::is_vchar;
use crate::utils::{is_nonascii_or, take_utf8_while1};

// A quoted string contains bytes that satisfy `is_vchar` or are in `ascii::WS`.
#[derive(PartialEq, Default, Clone, ToStatic, ToStringFromPrint)]
pub struct QuotedString<'a>(pub Vec<Cow<'a, str>>);

impl<'a> fmt::Debug for QuotedString<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_tuple("QuotedString")
            .field(&self.0.iter().collect::<Vec<_>>())
            .finish()
    }
}

impl<'a> QuotedString<'a> {
    pub fn push(&mut self, e: &'a str) {
        self.0.push(Cow::Borrowed(e))
    }

    pub fn chars<'b>(&'b self) -> QuotedStringChars<'a, 'b> {
        QuotedStringChars { q: self, inner: QuotedStringCharsInner::NextFragment(0) }
    }
}
impl<'a> Print for QuotedString<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        print_quoted(fmt, self.chars())
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for QuotedString<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<QuotedString<'a>> {
        let mut chunks = Vec::new();
        u.arbitrary_loop(None, Some(10), |u| {
            let bytes = arbitrary_string_where(u, |c| {
                is_vchar(c) || ascii::WS_CHAR.contains(&c)
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
        self.chars().collect::<String>() == other.chars().collect::<String>()
    }
}

#[derive(Clone)]
pub struct QuotedStringChars<'a, 'b> {
    q: &'b QuotedString<'a>,
    inner: QuotedStringCharsInner<'b>,
}
#[derive(Clone)]
enum QuotedStringCharsInner<'a> {
    NextFragment(usize),
    FragmentChars(usize, std::str::Chars<'a>),
}

impl<'a, 'b> Iterator for QuotedStringChars<'a, 'b> {
    type Item = char;
    fn next(&mut self) -> Option<Self::Item> {
        use QuotedStringCharsInner::*;
        match &mut self.inner {
            NextFragment(idx) =>
                match self.q.0.get(*idx) {
                    Some(frag) => {
                        self.inner = FragmentChars(*idx, frag.chars());
                        self.next()
                    },
                    None =>
                        None
                },
            FragmentChars(idx, it) =>
                match it.next() {
                    Some(c) => Some(c),
                    None => {
                        self.inner = NextFragment(*idx + 1);
                        self.next()
                    }
                }
        }
    }
}

/// Quoted pair
///
/// ```abnf
///    quoted-pair     =   ("\" (VCHAR / WSP)) / obs-qp
///    obs-qp          =   "\" (%d0 / obs-NO-WS-CTL / LF / CR)
/// ```
/// We parse quoted pairs even more liberally, allowing any ASCII byte after
/// the backslash.
///
/// However, we only return `Some(_)` for quoted pairs that are valid
/// according to the strict syntax; other quoted pairs cannot be printed
/// back and wee chose to ignore them.
pub fn quoted_pair(input: &[u8]) -> IResult<&[u8], Option<&str>> {
    preceded(
        tag(&[ascii::BACKSLASH]),
        map(
            verify(take(1usize), |b: &[u8]| b[0].is_ascii()),
            |b: &[u8]|
            if is_strict_quoted_pair(b[0].into()) {
                Some(unsafe { str::from_utf8_unchecked(b) })
            } else {
                None
            }
        )
    )(input)
}

fn is_strict_quoted_pair(c: char) -> bool {
    is_vchar(c) || ascii::WS_CHAR.contains(&c)
}

/// Allowed characters in quote
///
/// ```abnf
///   qtext           =   %d33 /             ; Printable US-ASCII
///                       %d35-91 /          ;  characters not including
///                       %d93-126 /         ;  "\" or the quote character
///                       obs-qtext
/// ```
/// following RFC6532, also allows non-ascii UTF-8
fn is_strict_qtext(c: char) -> bool {
    is_nonascii_or(|c| {
        c == ascii::EXCLAMATION
            || (ascii::NUM..=ascii::LEFT_BRACKET).contains(&c)
            || (ascii::RIGHT_BRACKET..=ascii::TILDE).contains(&c)
    })(c)
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
fn qcontent(input: &[u8]) -> IResult<&[u8], Option<&str>> {
    alt((
        map(take_utf8_while1(is_strict_qtext), Some),
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
    I: IntoIterator<Item = char>
{
    let mut buf = [0u8; 4];
    fmt.write_bytes(b"\"");
    for c in data.into_iter() {
        let b = c.encode_utf8(&mut buf).as_bytes();
        if is_strict_qtext(c) {
            fmt.write_bytes(b);
        } else if ascii::WS_CHAR.contains(&c) {
            // NOTE: we can either output the whitespace as folding
            // whitespace or to escape it; we choose to output it as folding
            // whitespace which helps performing line folding.
            fmt.write_fws_bytes(b);
        } else if is_vchar(c) {
            fmt.write_bytes(&[b'\\']);
            fmt.write_bytes(b);
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
    use crate::print::tests::print_to_vec_with;

    #[test]
    fn test_quoted_string_parser() {
        assert_eq!(
            quoted_string(b" \"hello\\\"world\" ").unwrap().1,
            QuotedString(vec![
                "hello".into(),
                "\"".into(),
                "world".into(),
            ])
        );

        assert_eq!(
            quoted_string(b"\"hello\r\n world\""),
            Ok((
                &b""[..],
                QuotedString(vec![
                    "hello".into(),
                    " ".into(),
                    "world".into(),
                ])
            )),
        );

        assert_eq!(
            quoted_string(b"\"\t\""),
            Ok((
                &b""[..],
                QuotedString(vec![
                    "\t".into(),
                ])
            )),
        );
    }

    #[test]
    fn test_quoted_string_printer() {
        let out = print_to_vec_with(|f| {
            print_quoted(
                f,
                QuotedString(vec![
                    "hello".into(),
                    "\"".into(),
                    " world".into(),
                ]).chars()
            );
        });
        assert_eq!(out, b"\"hello\\\" world\"");
    }

    #[test]
    fn test_quoted_string_object() {
        assert_eq!(
            QuotedString(vec![
                "hello".into(),
                " ".into(),
                "world".into(),
            ])
            .to_string(),
            "\"hello world\"".to_string(),
        );
    }
}
