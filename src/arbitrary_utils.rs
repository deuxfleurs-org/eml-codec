use arbitrary::{Arbitrary, Unstructured, Result};
use crate::text::ascii;

pub fn arbitrary_vec_where<'a, F, T>(u: &mut Unstructured<'a>, pred: F) -> Result<Vec<T>>
where
    F: for<'b> Fn(&'b T) -> bool,
    T: Arbitrary<'a>
{
    let len = u.arbitrary_len::<T>()?;
    let mut v = Vec::with_capacity(len);
    for _ in 0..len {
        let x: T = u.arbitrary()?;
        if pred(&x) {
            v.push(x)
        } else {
            return Err(arbitrary::Error::IncorrectFormat)
        }
    }
    Ok(v)
}

pub fn arbitrary_vec_nonempty_where<'a, F, T>(u: &mut Unstructured<'a>, pred: F, default: T) -> Result<Vec<T>>
where
    F: for<'b> Fn(&'b T) -> bool,
    T: Arbitrary<'a>
{
    let mut v = arbitrary_vec_where(u, pred)?;
    if v.is_empty() {
        v.push(default)
    }
    Ok(v)
}

pub fn arbitrary_vec_nonempty<'a, T>(u: &mut Unstructured<'a>) -> Result<Vec<T>>
where
    T: Arbitrary<'a>
{
    let (mut v, last): (Vec<T>, T) = u.arbitrary()?;
    v.push(last);
    Ok(v)
}

pub fn arbitrary_string_where<'a, F>(u: &mut Unstructured<'a>, pred: F) -> Result<String>
where
    F: Fn(char) -> bool,
{
    let len = u.arbitrary_len::<char>()?;
    let mut s = String::with_capacity(len);
    for _ in 0..len {
        let c: char = u.arbitrary()?;
        if pred(c) {
            s.push(c)
        } else {
            return Err(arbitrary::Error::IncorrectFormat)
        }
    }
    Ok(s)
}

pub fn arbitrary_string_nonempty_where<'a, F>(u: &mut Unstructured<'a>, pred: F, default: char) -> Result<String>
where
    F: Fn(char) -> bool,
{
    let mut v = arbitrary_string_where(u, pred)?;
    if v.is_empty() {
        v.push(default)
    }
    Ok(v)
}

pub fn arbitrary_whitespace_nonempty(u: &mut Unstructured) -> Result<String> {
    let mut s = String::new();
    for _ in 0..=u.arbitrary_len::<u8>()? {
        let b: bool = u.arbitrary()?;
        s.push((if b { ascii::SP } else { ascii::HT }).into());
    }
    Ok(s)
}

pub fn arbitrary_shuffle<T>(u: &mut Unstructured, v: &mut Vec<T>) -> Result<()> {
    let mut to_permute = &mut v[..];
    while to_permute.len() > 1 {
        let idx = u.choose_index(to_permute.len())?;
        to_permute.swap(0, idx);
        to_permute = &mut to_permute[1..];
    }
    Ok(())
}
