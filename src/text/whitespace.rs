use crate::text::ascii;
use crate::text::encoding::{Context, encoded_word_plain};
use crate::text::quoted::quoted_pair;
use crate::text::utf8::{is_nonascii_or, space0_str, space1_str, take_utf8_while1};
#[cfg(feature = "tracing")]
use crate::utils::bytes_to_display_string;
use nom::{
    branch::alt,
    bytes::complete::tag,
    combinator::{map, opt, recognize},
    multi::many1,
    sequence::{pair, tuple},
    IResult,
    Parser,
};
#[cfg(feature = "tracing-recover")]
use tracing::warn;

/// Whitespace (space, new line, tab) content and
/// delimited content (eg. comment, line, sections, etc.)

/// Obsolete/Compatible CRLF
///
/// Theoretically, all lines must end with \r\n
/// but some mail servers like Dovecot support malformated emails,
/// for example with only \n eol. It works because
/// \r or \n is allowed nowhere else, so we also add this support.
/// XXX this comment is incorrect: \r and \n are allowed in
/// obs-unstruct (obsolete unstructured fields).
/// This means that using obs_crlf instead of CRLF (as done currently)
/// may parse unstructured inputs in a way that contradicts the spec.

pub fn obs_crlf(input: &[u8]) -> IResult<&[u8], &str> {
    map(
        alt((
            tag(ascii::CRLF),
            map(
                alt((
                    tag(&[ascii::LF]),
                    tag(ascii::CRCRLF),
                    tag(&[ascii::CR]),
                )),
                |input: &[u8]| {
                    #[cfg(feature = "tracing-recover")]
                    warn!(input = unsafe { str::from_utf8_unchecked(input) },
                          "best-effort line ending");
                    input
                }
            )
        )),
        |b: &[u8]|
        // SAFETY: CR and LF are ASCII characters
        unsafe { str::from_utf8_unchecked(b) }
    )(input)
}

/// ```abnf
/// fold_line = any *(1*(crlf WS) any) crlf
/// ```
/// If `full_line` equals false, assumes this is a suffix of a line and
/// allows the input to start with a folded white space or to be immediately
/// terminated by a newline.
pub fn foldable_line(full_line: bool) -> impl Fn(&[u8]) -> IResult<&[u8], &[u8]> {
    move |input| {
        use memchr::memchr2_iter;
        // best-effort: we prefer to parse \r\n to represent a newline,
        // but we also allow a single \r or \n to represent a newline
        let mut it = memchr2_iter(b'\r', b'\n', input);
        while let Some(i) = it.next() {
            if i == 0 && full_line {
                break // reject input
            }

            match (input[i], input.get(i+1), input.get(i+2)) {
                (b'\r', Some(b'\n'), Some(b' ' | b'\t')) => {
                    let _ = it.next();
                    continue;
                },
                (b'\r', Some(b'\n'), _) => {
                    return Ok((&input[i+2..], &input[0..i]))
                },
                (_b /* \r | \n */, Some(b' ' | b'\t'), _) => {
                    #[cfg(feature = "tracing-recover")]
                    warn!(input = bytes_to_display_string(&[_b]), "foldable: best-effort line ending");
                    continue;
                },
                _ =>
                    return Ok((&input[i+1..], &input[0..i])),
            }
        }
        Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Fail)))
    }
}

// XXX if we want to do spec-compliant line framing later
// ```abnf
// fold_line = any *(crlf WS any) crlf
// ```
// used for pre-parsing / framing
// pub fn foldable_line(input: &[u8]) -> IResult<&[u8], &[u8]> {
//    terminated(
//        recognize(
//            pair(
//                is_not(ascii::CRLF),
//                many0(tuple((tag(ascii::CRLF), space1, is_not(ascii::CRLF))))
//            )
//        ),
//        tag(ascii::CRLF),
//    )(input)
// }

// --- whitespaces and comments

// Note: WSP = SP / HTAB = %x20 / %x09
// nom::*::space0 = *WSP
// nom::*::space1 = 1*WSP

/// Permissive foldable white space
///
/// Folding white space are used for long headers splitted on multiple lines.
/// The obsolete syntax allowes multiple lines without content; it is implemented
/// for compatibility reasons (as mandated by the spec).
///
/// The parser returns the slices of whitespace characters that were parsed, without
/// the CRLF line breaks. When printed back, line breaks will only be inserted according
/// to the correct (non-obsolete) syntax.
///
// XXX: the current implementation does not look spec compliant; alternative proposal below
//
// FWS             =   ([*WSP CRLF] 1*WSP) /  obs-FWS
// obs-FWS         =   1*([CRLF] WSP)                  (from errata)
//
// these definitions are in fact equivalent to:
//
// FWS             =   1*(WSP / CRLF WSP)
// or, alternatively
// FWS             =   1*(1*WSP / CRLF 1*WSP)
//
// We implement the latter because it represents sequences of WSP more efficiently.
// pub fn fws(input: &[u8]) -> IResult<&[u8], Vec<&[u8]>> {
//     many1(alt((space1, preceded(tag(ascii::CRLF), space1))))(input)
// }
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = bytes_to_display_string(input)))
)]
pub fn fws(input: &[u8]) -> IResult<&[u8], Vec<&str>> {
    alt((
        many1(fold_marker).map(|v| v.into_iter().flatten().collect()),
        space1_str.map(|wsp| vec![wsp]),
    ))(input)
}
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = bytes_to_display_string(input)))
)]
fn fold_marker(input: &[u8]) -> IResult<&[u8], Vec<&str>> {
    let (input, wsp0) = space0_str(input)?;
    let (input, _) = obs_crlf(input)?;
    let (input, wsp) = space1_str(input)?;

    let mut res = vec![];
    if !wsp0.is_empty() {
        res.push(wsp0)
    }
    res.push(wsp);
    Ok((input, res))
}

/// Folding White Space with Comment
///
/// Note: we drop the comments for now...  
///
/// ```abnf
///   ctext           =   %d33-39 /          ; Printable US-ASCII
///                       %d42-91 /          ;  characters not including
///                       %d93-126 /         ;  "(", ")", or "\"
///                       obs-ctext
///
///   ccontent        =   ctext / quoted-pair / comment
///
///   comment         =   "(" *([FWS] ccontent) [FWS] ")"
///
///   CFWS            =   (1*([FWS] comment) [FWS]) / FWS
/// ```
///
/// Note: a naive implementation of the grammar above using the alt()
/// combinator can lead to exponential backtracking on some inputs
/// (e.g. "((((((((").
/// Exponential backtracking can be tackled using the cut() combinator,
/// however any recursive definition can still run into stack overflow
/// errors on deeply nested inputs.
///
/// This is why we resort to the the low-level iterative implementation
/// of `comment` and `comment_body` below.
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = bytes_to_display_string(input)))
)]
 pub fn cfws(input: &[u8]) -> IResult<&[u8], ()> {
     alt((comments, fws.map(|_| ())))(input)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = bytes_to_display_string(input)))
)]
pub fn comments(input: &[u8]) -> IResult<&[u8], ()> {
    let (input, _) = many1(tuple((opt(fws), comment)))(input)?;
    let (input, _) = opt(fws)(input)?;
    Ok((input, ()))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = bytes_to_display_string(input)))
)]
pub fn comment(input: &[u8]) -> IResult<&[u8], ()> {
    let (input, _) = tag("(")(input)?;
    let (input, ()) = comment_body(input)?;
    Ok((input, ()))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = bytes_to_display_string(input)))
)]
pub fn comment_body(input: &[u8]) -> IResult<&[u8], ()> {
    let mut nesting = 1;
    let mut cursor: &[u8] = input;
    loop {
        if let Ok((input, _)) = pair(opt(fws), tag(")"))(cursor) {
            nesting -= 1;
            if nesting == 0 {
                return Ok((input, ()))
            }
            cursor = input;
            continue;
        }
        let (input, _) = opt(fws)(cursor)?;
        let (input, enter_subcomment) = alt((
            tag("(").map(|_| true),
            alt((
                recognize(quoted_pair),
                recognize(encoded_word_plain(Context::Comment)),
                recognize(ctext),
            )).map(|_| false)
        ))(input)?;

        if enter_subcomment {
            nesting += 1;
        }

        cursor = input
    }
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "trace", fields(input = bytes_to_display_string(input)))
)]
 pub fn ctext(input: &[u8]) -> IResult<&[u8], &str> {
     take_utf8_while1(is_ctext)(input)
}

/// RFC6532: ctext includes non-ascii UTF-8
pub fn is_ctext(c: char) -> bool {
    is_nonascii_or(|c| is_restr_ctext(c) || is_obs_no_ws_ctl(c))(c)
}

/// Check if it's a comment text character
///
/// ```abnf
///   ctext           =   %d33-39 /          ; Printable US-ASCII
///                       %d42-91 /          ;  characters not including
///                       %d93-126 /         ;  "(", ")", or "\"
///                       obs-ctext
///```
pub fn is_restr_ctext(c: u8) -> bool {
    (ascii::EXCLAMATION..=ascii::SQUOTE).contains(&c)
        || (ascii::ASTERISK..=ascii::LEFT_BRACKET).contains(&c)
        || (ascii::RIGHT_BRACKET..=ascii::TILDE).contains(&c)
}

/// US ASCII control characters without effect
///
/// ```abnf
///   obs-NO-WS-CTL   =   %d1-8 /            ; US-ASCII control
///                       %d11 /             ;  characters that do not
///                       %d12 /             ;  include the carriage
///                       %d14-31 /          ;  return, line feed, and
///                       %d127              ;  white space characters
/// ```
pub fn is_obs_no_ws_ctl(c: u8) -> bool {
    (ascii::SOH..=ascii::BS).contains(&c)
        || c == ascii::VT
        || c == ascii::FF
        || (ascii::SO..=ascii::US).contains(&c)
        || c == ascii::DEL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_obs_crlf() {
        assert_eq!(obs_crlf(b"\rworld"), Ok((&b"world"[..], &"\r"[..])));
        assert_eq!(obs_crlf(b"\r\nworld"), Ok((&b"world"[..], &"\r\n"[..])));
        assert_eq!(obs_crlf(b"\nworld"), Ok((&b"world"[..], &"\n"[..])));
    }

    #[test]
    fn test_fws() {
        assert_eq!(fws(b"\r\n world"), Ok((&b"world"[..], vec![&" "[..]])));
        assert_eq!(fws(b" \r\n \r\n world"), Ok((&b"world"[..], vec![&" "[..], &" "[..], &" "[..]])));
        assert_eq!(fws(b" world"), Ok((&b"world"[..], vec![&" "[..]])));
        assert_eq!(fws(b" \t  \r\n  world"), Ok((&b"world"[..], vec![&" \t  "[..], &"  "[..]])));
        assert!(fws(b"\r\nFrom: test").is_err());
    }

    #[test]
    fn test_cfws() {
        assert_eq!(
            cfws(b"(A nice \\) chap) <pete(his account)@silly.test(his host)>"),
            Ok((
                &b"<pete(his account)@silly.test(his host)>"[..],
                ()
            ))
        );
        assert_eq!(
            cfws(b"(Chris's host.)public.example>,"),
            Ok((&b"public.example>,"[..], ()))
        );
        assert_eq!(
            cfws(b"(double (comment) is fun) wouch"),
            Ok((&b"wouch"[..], ()))
        );
        // assert_eq!(
        //     cfws(b"(unbalanced ( parens) wouch"),
        //     Ok((&b"wouch"[..], &b"(double (comment) is fun) "[..]))
        // );
        assert_eq!(
            cfws(b"(using (256/256 bits) (2048 bits))"),
            Ok((&b""[..], ()))
        );
    }

    #[test]
    fn test_cfws_encoded_word() {
        assert_eq!(
            cfws(b"(=?US-ASCII?Q?Keith_Moore?=)"),
            Ok((&b""[..], ())),
        );
    }

    #[test]
    fn test_foldable_line() {
        assert_eq!(
            foldable_line(true)(b"abc\r\n def\r\n   ghi\r\n"),
            Ok((&b""[..], &b"abc\r\n def\r\n   ghi"[..])),
        );

        // a line that starts with FWS
        assert_eq!(
            foldable_line(false)(b"\r\n abc\r\n"),
            Ok((&b""[..], &b"\r\n abc"[..])),
        );
        assert!(foldable_line(true)(b"\r\n abc\r\n").is_err());
        assert!(foldable_line(true)(b"\n foo\r\n").is_err());

        // obsolete folding
        assert_eq!(
            foldable_line(true)(b"xx\r\n \r\n abc\r\n   \r\n def\r\n"),
            Ok((&b""[..], &b"xx\r\n \r\n abc\r\n   \r\n def"[..])),
        );

        // empty line
        assert_eq!(
            foldable_line(false)(b"\r\n"),
            Ok((&b""[..], &b""[..])),
        );
        assert_eq!(
            foldable_line(false)(b"\n"),
            Ok((&b""[..], &b""[..])),
        );
        assert!(foldable_line(true)(b"\r\n").is_err());
        assert!(foldable_line(true)(b"\n").is_err());
    }
}
