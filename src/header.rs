use bounded_static::ToStatic;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_while1},
    character::complete::space0,
    combinator::{all_consuming, eof, map, rest},
    multi::many0,
    sequence::{pair, terminated, tuple},
    IResult, Parser,
};
use std::borrow::Cow;
use std::fmt;

use crate::print::{Print, Formatter};
use crate::text::misc_token;
use crate::text::whitespace::{foldable_line, obs_crlf};

// A valid header field name.
#[derive(PartialEq, Clone, ToStatic)]
pub struct FieldName<'a>(pub Cow<'a, [u8]>);
impl<'a> FieldName<'a> {
    pub fn bytes(&'a self) -> &'a [u8] {
        &self.0
    }
}
impl<'a> fmt::Debug for FieldName<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_tuple("header::FieldName")
            .field(&String::from_utf8_lossy(&self.0))
            .finish()
    }
}
impl<'a> Print for FieldName<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        fmt.write_bytes(&self.0)
    }
}

// Intermediate AST for two-step parsing of header fields. Structured headers
// are then parsed from this.
//
// A `FieldRaw` corresponds to a header field after performing "framing", i.e.
// identifier header field boundaries: it is the raw data found between two
// header boundaries.
//
// - `Good` corresponds to a header field that could be split into a
// valid name and arbitrary body. It does not say anything about the validity of
// the body. The body is stored as a raw slice because it will be parsed further.
//
// - `Bad` corresponds to a header field that could not be split into a name and
// body; it basically contains arbitrary data.
#[derive(PartialEq, Clone)]
pub struct FieldRaw<'a> {
    pub name: FieldName<'a>,
    pub body: &'a [u8],
}
impl<'a> fmt::Debug for FieldRaw<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt
            .debug_struct("header::FieldRaw")
            .field("name", &self.name)
            .field("body", &String::from_utf8_lossy(&self.body))
            .finish()
    }
}

/// Parse headers as raw key/values.
/// Stop at an empty line or at EOF.
pub fn header_kv(input: &[u8]) -> (&[u8], Vec<FieldRaw<'_>>) {
    // SAFETY: both `field_raw` and `foldable_line` only accept non-empty inputs
    let (input, mut fields) = many0(field_raw_opt)(input).unwrap();
    // SAFETY: `rest` (last case) always succeeds.
    let (input, terminator) = alt((
        // empty line
        map(obs_crlf, |_| None),
        // The empty line is optional if there is no body following the headers,
        // so we must also accept EOF.
        map(eof, |_| None),
        // For best-effort parsing, we also try to parse any remaining bytes before
        // EOF (as if EOF was a CRLF).
        map(pair(field_name, rest), |(name, body)| Some(FieldRaw { name, body })),
        map(rest, |_| None),
    ))(input).unwrap();

    fields.push(terminator);

    // drop `None`s ("bad" fields)
    let fields = fields.into_iter().filter_map(|f| f).collect();

    (input, fields)
}

// NOTE: field_raw only recognizes non-empty inputs.
fn field_raw(input: &[u8]) -> IResult<&[u8], FieldRaw<'_>> {
    map(
        pair(field_name, foldable_line),
        |(name, body)| FieldRaw { name, body }
    )(input)
}

// A best-effort version of `field_raw` that also recognizes lines that cannot
// be parsed as a field name and body. (It returns `None` in this case.)
// NOTE: `field_raw_opt` only recognizes non-empty inputs.
// NOTE: furthermore, in the "best effort" case, `foldable_line` always
// recognizes non-empty lines; this is important so that it does not consume the
// final empty line (obs_crlf) that terminates `header_kv`.
fn field_raw_opt(input: &[u8]) -> IResult<&[u8], Option<FieldRaw<'_>>> {
    alt((
        map(field_raw, Some),
        // best-effort: a foldable line that cannot even be parsed as a field name
        // and body. We drop it afterwards.
        map(foldable_line, |_| None),
    ))(input)
}

/// Header field name
/// ```abnf
/// field-name =   1*ftext
/// ftext      =   %d33-57 /          ; Printable US-ASCII
///                %d59-126           ;  characters not including
///                                   ;  ":".
/// followed by *WSP in the obsolete syntax
/// ```
pub fn field_name(input: &[u8]) -> IResult<&[u8], FieldName<'_>> {
    terminated(
        take_while1(|c| (0x21..=0x7E).contains(&c) && c != 0x3A)
            .map(|s| FieldName(Cow::Borrowed(s))),
        tuple((space0, tag(b":"))),
    )(input)
}

// Parse a raw header field as an unstructured header

#[derive(Debug, PartialEq, Clone, ToStatic)]
pub struct Unstructured<'a>(pub FieldName<'a>, pub misc_token::Unstructured<'a>);

impl<'a> Unstructured<'a> {
    // TODO: don't throw away the errors
    pub fn from_raw(f: FieldRaw<'a>) -> Option<Unstructured<'a>> {
        let (_, body) = all_consuming(misc_token::unstructured)(f.body).ok()?;
        Some(Unstructured(f.name, body))
    }
}
impl<'a> Print for Unstructured<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        print_unstructured(fmt, &self.0.0, &self.1)
    }
}

// Helper to print structured headers

pub fn print<T: Print>(fmt: &mut impl Formatter, name: &[u8], body: T) {
    fmt.write_bytes(name);
    fmt.write_bytes(b":");
    fmt.write_fws();
    body.print(fmt);
    fmt.write_crlf();
}

pub fn print_unstructured<'a>(fmt: &mut impl Formatter, name: &[u8], body: &misc_token::Unstructured<'a>) {
    fmt.write_bytes(name);
    fmt.write_bytes(b":");
    body.print(fmt);
    fmt.write_crlf();
}

#[cfg(test)]
mod tests {
    use super::*;
    use misc_token::{UnstrToken, UnstrTxtKind};

    #[test]
    fn test_field_raw_good() {
        let (rest, f) = field_raw(b"X-Unknown: something something\r\n").unwrap();
        assert!(rest.is_empty());
        assert_eq!(
            f,
            FieldRaw {
                name: FieldName(b"X-Unknown".into()),
                body: &b" something something"[..],
            }
        );
    }

    #[test]
    fn test_unstructured() {
        let u = Unstructured::from_raw(
            FieldRaw {
                name: FieldName(b"X-Unknown".into()),
                body: &b" something something"[..],
            }
        )
        .unwrap();
        assert_eq!(
            u,
            Unstructured(
                FieldName(b"X-Unknown".into()),
                misc_token::Unstructured(vec![
                    UnstrToken::from_plain(b" ", UnstrTxtKind::Fws),
                    UnstrToken::from_plain(b"something", UnstrTxtKind::Txt),
                    UnstrToken::from_plain(b" ", UnstrTxtKind::Fws),
                    UnstrToken::from_plain(b"something", UnstrTxtKind::Txt),
                ])
            )
        )
    }

    #[test]
    fn test_no_body() {
        let (rest, fields) = header_kv(b"X-Foo: something something\r\nX-Bar: something else\r\n");
        assert!(rest.is_empty());
        assert_eq!(
            fields,
            vec![
                FieldRaw { name: FieldName(b"X-Foo".into()), body: b" something something" },
                FieldRaw { name: FieldName(b"X-Bar".into()), body: b" something else" },
            ]
        )
    }

    #[test]
    fn test_best_effort_good_before_eof() {
        let (rest, fields) = header_kv(b"X-Foo: something something\r\nX-Bar: something else");
        assert!(rest.is_empty());
        assert_eq!(
            fields,
            vec![
                FieldRaw { name: FieldName(b"X-Foo".into()), body: b" something something" },
                FieldRaw { name: FieldName(b"X-Bar".into()), body: b" something else" },
            ]
        )
    }

    #[test]
    fn test_best_effort_bad_before_eof() {
        let (rest, fields) = header_kv(b"X-Foo: something something\r\nrandom junk");
        assert!(rest.is_empty());
        assert_eq!(
            fields,
            vec![
                FieldRaw { name: FieldName(b"X-Foo".into()), body: b" something something" },
            ]
        )
    }
}
