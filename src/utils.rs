pub(crate) fn set_opt<T>(o: &mut Option<T>, x: T) -> bool {
    match *o {
        None => { *o = Some(x); true },
        Some(_) => false,
    }
}

pub(crate) fn append_opt<T>(o: &mut Option<Vec<T>>, x: Vec<T>) -> bool {
    match o {
        None => { *o = Some(x); true },
        Some(v) => { v.extend(x); false },
    }
}

pub(crate) fn vec_filter_none_nonempty<T>(v: Vec<Option<T>>) -> Option<Vec<T>> {
    let v: Vec<T> = v.into_iter().flatten().collect();
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

#[allow(dead_code)]
pub fn bytes_to_display_string(bs: &[u8]) -> String {
    let mut s = String::new();
    s.push('"');
    for b in bs {
        match b {
            b'\\' => s.push_str("\\\\"),
            b if b.is_ascii_alphanumeric() ||
                b.is_ascii_graphic() ||
                *b == b' ' =>
                s.push(*b as char),
            b'\"' => s.push_str("\\\""),
            b'\r' => s.push_str("\\r"),
            b'\n' => s.push_str("\\n"),
            b'\t' => s.push_str("\\t"),
            _ => s.push_str(&format!("\\{}", b)),
        }
    }
    s.push('"');
    s
}

// NOTE: should we try to recover on invalid UTF-8, by replacing invalid data
// with std::char::REPLACEMENT_CHAR? The RFC doesn't require this however, and
// it would complicate the output type of the function (it would not be a single
// contiguous slice of the input).
pub fn take_utf8_while1<F>(cond: F) -> impl Fn(&[u8]) -> nom::IResult<&[u8], &str>
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
            Ok((rest, unsafe { str::from_utf8_unchecked(&i[0..delta]) }))
        } else {
            Err(nom::Err::Error(nom::error::Error {
                input: i,
                code: nom::error::ErrorKind::TakeWhile1,
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
    let (input, sp) = nom::character::complete::space0(input)?;
    Ok((input, unsafe { str::from_utf8_unchecked(sp) }))
}

pub fn space1_str(input: &[u8]) -> nom::IResult<&[u8], &str> {
    let (input, sp) = nom::character::complete::space1(input)?;
    Ok((input, unsafe { str::from_utf8_unchecked(sp) }))
}
