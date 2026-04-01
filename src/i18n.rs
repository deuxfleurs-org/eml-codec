// Import the derive macro for ContainsUtf8
pub use eml_codec_derives::ContainsUtf8;

/// The `contains_utf8` function returns whether a value implementing this trait
/// uses non-ascii UTF-8 text added by RFC 6532.
///
/// This is useful to distinguish between headers bodies or other tokens that
/// require RFC 6532 and those that do not.
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
