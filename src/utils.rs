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

use nom::error::ParseError;
use nom::IResult;
use nom::{FindToken, InputTakeAtPosition};

/// Like `nom::bytes::complete::is_not`, but allows the pattern to be
/// at the beginning of the input...
pub fn is_not0<T, Input, Error: ParseError<Input>>(
  arr: T,
) -> impl Fn(Input) -> IResult<Input, Input, Error>
where
  Input: InputTakeAtPosition,
  T: FindToken<<Input as InputTakeAtPosition>::Item>,
{
  move |i: Input| {
    i.split_at_position_complete(|c| arr.find_token(c))
  }
}
