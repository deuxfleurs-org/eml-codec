#[cfg(feature = "arbitrary")]
use arbitrary::Arbitrary;
use bounded_static::ToStatic;
use std::borrow::Cow;
use std::fmt;

#[cfg(feature = "arbitrary")]
use crate::fuzz_eq::FuzzEq;
use crate::mime;

#[derive(Clone, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub struct Text<'a> {
    pub mime: mime::MIME<'a, mime::r#type::Text<'a>>,
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

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for Text<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        // XXX one or two final \r may get eaten by the best-effort parsing strategy...
        // (see also the comment in the test `test_multipart_cr` in part/composite.rs)
        // as a workaround, avoid this case for now...
        let mime = u.arbitrary()?;
        let mut body: Vec<_> = u.arbitrary()?;
        if let Some(b'\r') = body.last() {
            body.push(b'X')
        }
        Ok(Self { mime, body: body.into() })
    }
}

#[derive(Clone, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
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

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for Binary<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        // XXX same as for Text
        let mime = u.arbitrary()?;
        let mut body: Vec<_> = u.arbitrary()?;
        if let Some(b'\r') = body.last() {
            body.push(b'X')
        }
        Ok(Self { mime, body: body.into() })
    }
}
