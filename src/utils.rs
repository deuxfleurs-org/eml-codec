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

pub use eml_codec_derives::ContainsUtf8;

// This is a trait instead of plain old functions because it allows factoring
// code when working on instances of MIME<'a, T> for specific Ts
pub trait ContainsUtf8 {
    fn contains_utf8(&self) -> bool;
}
impl<T: ContainsUtf8> ContainsUtf8 for Option<T> {
    fn contains_utf8(&self) -> bool {
        match &self {
            None => false,
            Some(x) => x.contains_utf8(),
        }
    }
}
impl<T: ContainsUtf8> ContainsUtf8 for Box<T> {
    fn contains_utf8(&self) -> bool {
        <T as ContainsUtf8>::contains_utf8(self.as_ref())
    }
}
impl<'a, T: ContainsUtf8> ContainsUtf8 for Vec<T> {
    fn contains_utf8(&self) -> bool {
        self.iter().any(|x| x.contains_utf8())
    }
}
impl<'a> ContainsUtf8 for std::borrow::Cow<'a, str> {
    fn contains_utf8(&self) -> bool {
        self.as_bytes().iter().any(|b| !b.is_ascii())
    }
}
