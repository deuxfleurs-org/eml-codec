#[cfg(feature = "arbitrary")]
use arbitrary::Arbitrary;
use bounded_static::ToStatic;
use std::borrow::Cow;
use std::fmt;

use crate::mime;
use crate::raw_input::RawInput;
#[cfg(feature = "arbitrary")]
use crate::{arbitrary_utils::arbitrary_part_body, fuzz_eq::FuzzEq};

#[derive(Clone, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub struct Text<'a> {
    pub mime: mime::MIME<'a, mime::r#type::Text<'a>>,
    #[cfg_attr(feature = "arbitrary", fuzz_eq(use_eq))]
    pub body: Cow<'a, [u8]>,
    pub raw_body: RawInput<'a>,
}

impl<'a> fmt::Debug for Text<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("part::Text")
            .field("mime", &self.mime)
            .field("body", &String::from_utf8_lossy(&self.body))
            .field("raw_body", &self.raw_body)
            .finish()
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for Text<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self {
            mime: u.arbitrary()?,
            body: arbitrary_part_body(u)?.into(),
            raw_body: RawInput::none(),
        })
    }
}

#[derive(Clone, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub struct Binary<'a> {
    pub mime: mime::MIME<'a, mime::r#type::Binary<'a>>,
    #[cfg_attr(feature = "arbitrary", fuzz_eq(use_eq))]
    pub body: Cow<'a, [u8]>,
    pub raw_body: RawInput<'a>,
}

impl<'a> fmt::Debug for Binary<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("part::Binary")
            .field("mime", &self.mime)
            .field("body", &String::from_utf8_lossy(&self.body))
            .field("raw_body", &self.raw_body)
            .finish()
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for Binary<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self {
            mime: u.arbitrary()?,
            body: arbitrary_part_body(u)?.into(),
            raw_body: RawInput::none(),
        })
    }
}
