use nom::{
    character::complete::{space0, space1},
    error::{Error, ErrorKind},
    Err,
    IResult
};
#[cfg(feature = "tracing-recover")]
use tracing::warn;
use std::borrow::Cow;
#[cfg(feature = "tracing-recover")]
use crate::utils::bytes_to_trace_string;

/// Parses the input as a sequence of UTF-8 characters that satisfy the
/// predicate `cond`. If invalid UTF-8 is encountered, it is replaced by
/// [`char::REPLACEMENT_CHARACTER`] and parsing continues.
///
/// This function is zero-copy if the parsed input is valid UTF-8, otherwise a
/// string gets allocated because of the need to insert replacement characters.
/// This is similar to how [`String::from_utf8_lossy`] works.
pub fn take_utf8_while1<F>(cond: F) -> impl Fn(&[u8]) -> IResult<&[u8], Cow<'_, str>>
    where F: Fn(char) -> bool
{
    move |i: &[u8]| {
        let mut it = utf8_iter::ErrorReportingUtf8Chars::new(i);
        let i_len = i.len();
        let mut rest = i;
        // read first chunk of valid UTF-8
        loop {
            match it.next() {
                Some(Ok(c)) if cond(c) => {
                    rest = it.as_slice();
                },
                Some(Err(_)) => {
                    // encountered invalid UTF-8
                    break
                },
                _ => {
                    // end of input or cond() returned false; stop reading.
                    //
                    // NOTE: we are careful of using `rest` and not
                    // `it.as_slice()` to denote the rest of the input: if we
                    // just read a character for which cond() is false, then
                    // this character has already been returned by the iterator
                    // and is not part of it.as_slice() (but it is part of
                    // `rest`, which is only advanced in the `Some(Ok(c)) if
                    // cond(c)` branch above).
                    let end = i_len - rest.len();
                    if end > 0 {
                        // SAFETY: `0..end` represents a subslice in which the
                        // `utf8_iter` iterator recognized strictly valid UTF-8
                        // codepoints. (We use the `ErrorReportingUtf8Chars`
                        // iterator and break out of the loop as soon as it
                        // encounters bytes that are not valid UTF-8.)
                        let sub = unsafe { str::from_utf8_unchecked(&i[0..end]) };
                        return Ok((rest, Cow::Borrowed(sub)))
                    } else {
                        return Err(Err::Error(Error {
                            input: i,
                            code: ErrorKind::TakeWhile1,
                        }))
                    }
                }
            }
        }

        // we have encountered some invalid UTF-8.
        #[cfg(feature = "tracing-recover")]
        warn!(input = %bytes_to_trace_string(i), "input contains invalid UTF-8");

        let mut s = String::new();
        // SAFETY: `0..end` only contains bytes on which the iterator
        // returned Ok (same as above).
        s.push_str(unsafe { str::from_utf8_unchecked(&i[0..i_len - rest.len()]) });
        // push a replacement for the invalid UTF-8
        s.push(char::REPLACEMENT_CHARACTER);

        // read remaining valid and invalid text, pushing it to `s`.
        let mut start = i_len - it.as_slice().len();
        let mut rest = it.as_slice();
        loop {
            match it.next() {
                Some(Ok(c)) if cond(c) => {
                    rest = it.as_slice();
                },
                res => {
                    // invalid utf8, end of input, or cond() returned false

                    // start by pushing the valid chunk read so far
                    let end = i_len - rest.len();
                    // SAFETY: `start..end` only contains bytes on which the iterator
                    // return Ok()
                    s.push_str(unsafe { str::from_utf8_unchecked(&i[start..end]) });

                    if let Some(Err(_)) = res {
                        // if we read invalid utf8, push a replacement and continue
                        s.push(char::REPLACEMENT_CHARACTER);
                        start = i_len - it.as_slice().len();
                        rest = it.as_slice();
                    } else {
                        // otherwise, stop reading
                        break
                    }
                },
            }
        }

        if !s.is_empty() {
            Ok((rest, Cow::Owned(s)))
        } else {
            Err(Err::Error(Error { input: i, code: ErrorKind::TakeWhile1 }))
        }
    }
}

pub fn is_nonascii_or<F>(cond: F) -> impl Fn(char) -> bool
    where F: Fn(u8) -> bool
{
    move |c: char| {
        if c.is_ascii() {
            let c = u8::try_from(c).unwrap();
            cond(c)
        } else {
            true
        }
    }
}

pub fn is_ascii_and<F>(cond: F) -> impl Fn(char) -> bool
    where F: Fn(u8) -> bool
{
    move |c: char| {
        if c.is_ascii() {
            let c = u8::try_from(c).unwrap();
            cond(c)
        } else {
            false
        }
    }
}

pub fn space0_str(input: &[u8]) -> nom::IResult<&[u8], &str> {
    let (input, sp) = space0(input)?;
    // SAFETY: the `space0` combinator recognizes sequences of ' ' and '\t',
    // which are ASCII.
    Ok((input, unsafe { str::from_utf8_unchecked(sp) }))
}

pub fn space1_str(input: &[u8]) -> nom::IResult<&[u8], &str> {
    let (input, sp) = space1(input)?;
    // SAFETY: the `space1` combinator recognizes sequences of ' ' and '\t',
    // which are ASCII.
    Ok((input, unsafe { str::from_utf8_unchecked(sp) }))
}
