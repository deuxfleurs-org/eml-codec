use std::fmt;

use crate::mime;

#[derive(PartialEq)]
pub struct Text<'a> {
    pub interpreted: mime::mime::Text<'a>,
    pub body: &'a [u8],
}

impl<'a> fmt::Debug for Text<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("part::Text")
            .field("mime", &self.interpreted)
            .field(
                "body",
                &format_args!("\"{}\"", String::from_utf8_lossy(self.body)),
            )
            .finish()
    }
}

#[derive(PartialEq)]
pub struct Binary<'a> {
    pub interpreted: mime::mime::Binary<'a>,
    pub body: &'a [u8],
}

impl<'a> fmt::Debug for Binary<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("part::Binary")
            .field("mime", &self.interpreted)
            .field(
                "body",
                &format_args!("\"{}\"", String::from_utf8_lossy(self.body)),
            )
            .finish()
    }
}
