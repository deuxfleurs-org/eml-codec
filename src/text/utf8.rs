use nom::{
    character::complete::{space0, space1},
    error::{Error, ErrorKind},
    Err,
    IResult
};

// NOTE: should we try to recover on invalid UTF-8, by replacing invalid data
// with std::char::REPLACEMENT_CHAR? The RFC doesn't require this however, and
// it would complicate the output type of the function (it would not be a single
// contiguous slice of the input).
pub fn take_utf8_while1<F>(cond: F) -> impl Fn(&[u8]) -> IResult<&[u8], &str>
    where F: Fn(char) -> bool
{
    move |i: &[u8]| {
        let mut it = utf8_iter::ErrorReportingUtf8Chars::new(i);
        let mut rest = i;
        while let Some(Ok(c)) = it.next() {
            if !cond(c) {
                break
            }
            rest = it.as_slice()
        }
        let delta = i.len() - rest.len();
        if delta > 0 {
            // SAFETY: `0..delta` represents a subslice in which the `utf8_iter`
            // iterator recognized strictly valid UTF-8 codepoints. (We use the
            // `ErrorReportingUtf8Chars` iterator that returns Err() when it
            // encounters bytes that are not valid UTF-8.)
            Ok((rest, unsafe { str::from_utf8_unchecked(&i[0..delta]) }))
        } else {
            Err(Err::Error(Error {
                input: i,
                code: ErrorKind::TakeWhile1,
            }))
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
