use arbitrary::{Arbitrary, Unstructured, Result};
use std::ops::ControlFlow;
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

pub fn arbitrary_vec_nonempty<'a, T>(u: &mut Unstructured<'a>) -> Result<Vec<T>>
where
    T: Arbitrary<'a>
{
    let (mut v, last): (Vec<T>, T) = u.arbitrary()?;
    v.push(last);
    Ok(v)
}

// generate simple FWS and obs-FWS
pub fn arbitrary_fws(u: &mut Unstructured) -> Result<Vec<u8>> {
    let mut v = Vec::new();
    u.arbitrary_loop(Some(1), Some(3), |u| {
        if u.arbitrary()? {
            v.extend(ascii::CRLF)
        }
        for _ in 0..u.int_in_range(1..=4)? {
            v.push(b' ')
        }
        Ok(ControlFlow::Continue(()))
    })?;
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
