use bounded_static::ToStatic;
use std::borrow::Cow;
use std::fmt;

use crate::mime;

#[derive(PartialEq, ToStatic)]
pub struct Text<'a> {
    pub mime: mime::MIME<'a, mime::r#type::DeductibleText>,
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

#[derive(PartialEq, ToStatic)]
pub struct Binary<'a> {
    pub mime: mime::MIME<'a, mime::r#type::Binary>,
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
