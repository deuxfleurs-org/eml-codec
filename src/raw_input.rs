#[cfg(feature = "arbitrary")]
use crate::fuzz_eq::FuzzEq;
use crate::i18n::ContainsUtf8;
#[cfg(feature = "arbitrary")]
use arbitrary::Arbitrary;
use bounded_static::{IntoBoundedStatic, ToBoundedStatic};
use std::borrow::Cow;
use std::fmt;

#[derive(Clone, PartialEq)]
pub struct RawInput<'a>(pub Option<&'a [u8]>);

impl<'a> RawInput<'a> {
    pub fn none() -> RawInput<'static> {
        RawInput(None)
    }

    pub fn unwrap(&self) -> &'a [u8] {
        match self.0 {
            None => panic!("Called RawInput::unwrap() on a RawInput::none()"),
            Some(s) => s,
        }
    }
}

impl<'a> From<&'a [u8]> for RawInput<'a> {
    fn from(s: &'a [u8]) -> Self {
        Self(Some(s))
    }
}

impl<'a, const N: usize> From<&'a [u8; N]> for RawInput<'a> {
    fn from(s: &'a [u8; N]) -> Self {
        Self(Some(s))
    }
}

impl<'a> fmt::Debug for RawInput<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            None => fmt.debug_tuple("None").finish(),
            Some(s) => {
                let maxlen = 100;
                let disp: Cow<[u8]> = if s.len() <= maxlen {
                    Cow::Borrowed(s)
                } else {
                    let mut disp = vec![];
                    disp.extend_from_slice(&s[0..maxlen / 2]);
                    disp.extend_from_slice(b"..");
                    disp.extend_from_slice(&s[s.len() - (maxlen / 2)..s.len()]);
                    Cow::Owned(disp)
                };
                fmt.debug_tuple("Some")
                    .field(&String::from_utf8_lossy(&disp))
                    .finish()
            }
        }
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for RawInput<'a> {
    fn arbitrary(_u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(RawInput(None))
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> FuzzEq for RawInput<'a> {
    /// `RawInput` data is always informational and never part of the proper
    /// "canonical" AST data. We thus ignore its content when comparing it with
    /// fuzz_eq.
    fn fuzz_eq(&self, _other: &Self) -> bool {
        true
    }
}

// Moving to an owned version discards the reference to the input.
impl<'a> IntoBoundedStatic for RawInput<'a> {
    type Static = RawInput<'static>;
    fn into_static(self) -> Self::Static {
        RawInput(None)
    }
}
impl<'a> ToBoundedStatic for RawInput<'a> {
    type Static = RawInput<'static>;
    fn to_static(&self) -> Self::Static {
        RawInput(None)
    }
}

// Ignore RawInput values wrt ContainsUtf8
impl<'a> ContainsUtf8 for RawInput<'a> {
    fn contains_utf8(&self) -> bool {
        false
    }
}

#[cfg(test)]
impl<'a> RawInput<'a> {
    pub(crate) fn between(input: &'a [u8], prefix: &[u8], suffix: &[u8]) -> Self {
        use memchr::memmem;
        let prefix_matches: Vec<_> = memmem::find_iter(input, prefix).collect();
        if prefix_matches.len() != 1 {
            panic!("{} prefix matches (expected: 1)", prefix_matches.len());
        }
        let prefix_pos = prefix_matches[0];
        let suffix_matches: Vec<_> =
            memmem::find_iter(&input[prefix_pos + prefix.len()..], suffix).collect();
        if suffix_matches.len() != 1 {
            panic!("{} suffix matches (expected: 1)", suffix_matches.len());
        }
        let suffix_pos = suffix_matches[0] + prefix_pos + prefix.len();
        RawInput(Some(&input[prefix_pos..suffix_pos + suffix.len()]))
    }

    pub(crate) fn between_excl(input: &'a [u8], before: &[u8], after: &[u8]) -> Self {
        use memchr::memmem;
        let before_matches: Vec<_> = memmem::find_iter(input, before).collect();
        if before_matches.len() != 1 {
            panic!("{} before matches (expected: 1)", before_matches.len());
        }
        let before_pos = before_matches[0];
        let after_matches: Vec<_> =
            memmem::find_iter(&input[before_pos + before.len()..], after).collect();
        if after_matches.len() != 1 {
            panic!("{} after matches (expected: 1)", after_matches.len());
        }
        let after_pos = after_matches[0] + before_pos + before.len();
        RawInput(Some(&input[before_pos + before.len()..after_pos]))
    }
}
