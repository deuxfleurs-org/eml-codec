#[cfg(feature = "arbitrary")]
use arbitrary::Arbitrary;
use bounded_static::ToStatic;
use std::borrow::Cow;
use std::fmt;

#[cfg(feature = "arbitrary")]
use crate::fuzz_eq::FuzzEq;
use crate::mime;

#[derive(Clone, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary, FuzzEq))]
pub struct Text<'a> {
    pub mime: mime::MIME<'a, mime::r#type::DeductibleText<'a>>,
    #[cfg_attr(feature = "arbitrary", fuzz_eq(use_eq))]
    pub body: Cow<'a, [u8]>,
}

impl<'a> fmt::Debug for Text<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("part::Text")
            .field("mime", &self.mime)
            .field("body", &String::from_utf8_lossy(&self.body))
            .finish()
    }
}

#[derive(Clone, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary, FuzzEq))]
pub struct Binary<'a> {
    pub mime: mime::MIME<'a, mime::r#type::Binary<'a>>,
    #[cfg_attr(feature = "arbitrary", fuzz_eq(use_eq))]
    pub body: Cow<'a, [u8]>,
}

impl<'a> fmt::Debug for Binary<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("part::Binary")
            .field("mime", &self.mime)
            .field("body", &String::from_utf8_lossy(&self.body))
            .finish()
    }
}
