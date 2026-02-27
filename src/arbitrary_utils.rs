use arbitrary::{Arbitrary, Unstructured, Result};
use crate::text::ascii;

pub fn arbitrary_vec_where<'a, F, T>(u: &mut Unstructured<'a>, pred: F) -> Result<Vec<T>>
where
    F: Fn(T) -> bool,
    T: Arbitrary<'a> + Copy
{
    let len = u.arbitrary_len::<T>()?;
    let mut v = Vec::with_capacity(len);
    for _ in 0..len {
        let x: T = u.arbitrary()?;
        if pred(x) {
            v.push(x)
        } else {
            break;
        }
    }
    Ok(v)
}

pub fn arbitrary_vec_nonempty_where<'a, F, T>(u: &mut Unstructured<'a>, pred: F, default: T) -> Result<Vec<T>>
where
    F: Fn(T) -> bool,
    T: Arbitrary<'a> + Copy
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

pub fn arbitrary_whitespace_nonempty(u: &mut Unstructured) -> Result<Vec<u8>> {
    let mut v = Vec::new();
    for _ in 0..=u.arbitrary_len::<u8>()? {
        let b: bool = u.arbitrary()?;
        v.push(if b { ascii::SP } else { ascii::HT });
    }
    Ok(v)
}

pub fn arbitrary_shuffle<T>(u: &mut Unstructured, v: &mut Vec<T>) {
    let mut to_permute = &mut v[..];
    while to_permute.len() > 1 {
        let idx = u.choose_index(to_permute.len()).unwrap();
        to_permute.swap(0, idx);
        to_permute = &mut to_permute[1..];
    }
}
