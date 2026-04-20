/// Parts that contain other parts inside them
pub mod composite;

/// Parts that have a body and no child parts
pub mod discrete;

/// Representation of all headers in a MIME entity
pub mod field;

#[cfg(feature = "arbitrary")]
use arbitrary::Arbitrary;
#[cfg(feature = "tracing-unsupported")]
use tracing::warn;
use bounded_static::ToStatic;
use std::borrow::Cow;
#[cfg(feature = "arbitrary")]
use crate::{
    header,
    arbitrary_utils::{arbitrary_shuffle, arbitrary_vec_where},
    fuzz_eq::FuzzEq,
    mime,
};
#[cfg(feature = "tracing-unsupported")]
use crate::utils::bytes_to_trace_string;
use crate::i18n::ContainsUtf8;
use crate::mime::AnyMIME;
use crate::part::{
    composite::{message, multipart, Message, Multipart},
    discrete::{Binary, Text},
};
use crate::print::{print_seq, Print, Formatter};
use crate::raw_input::RawInput;

#[derive(Clone, Debug, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub struct AnyPart<'a> {
    // Invariant: `fields` must be "complete and correct":
    // - it must contain an entry for every piece of information contained in
    //   `mime_body`'s mime headers that is not the default value. (This means
    //    values of which are `Deductible::Explicit` or optionals set to
    //    `Some(_)`.)
    // - it must *only* contain entries for fields that have a value. (This means
    //   no optional fields set to `None`.)
    // Invariant: `fields` must contain no duplicates.
    pub entries: Vec<field::EntityEntry<'a>>,
    pub mime_body: MimeBody<'a>,
    pub raw: RawInput<'a>,
    pub raw_headers: RawInput<'a>,
}

impl<'a> AnyPart<'a> {
    pub fn contains_utf8_headers(&self) -> bool {
        self.entries.iter().find(|f| {
            match f {
                field::EntityEntry::Unstructured(u) => u.contains_utf8(),
                _ => false,
            }
        }).is_some()
        ||
        self.mime_body.mime().contains_utf8()
    }

    // TODO: return an iterator instead of a Vec?
    pub fn field_list(&self) -> Vec<field::EntityField<'a>> {
        let mime = self.mime_body.mime();
        let mut v = vec![];
        for e in &self.entries {
            let field = match e {
                field::EntityEntry::MIME(e) => {
                    // SAFETY: `self.entries` must only contain entries for
                    // fields that are actually present in `mime`.
                    field::EntityField::MIME(mime.get_field(*e).unwrap())
                },
                field::EntityEntry::Unstructured(u) =>
                    field::EntityField::Unstructured(u.clone()),
            };
            v.push(field);
        }
        v
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for AnyPart<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let mime_body: MimeBody = u.arbitrary()?;
        let mut entries: Vec<field::EntityEntry> =
            mime_body.mime()
                     .field_entries()
                     .into_iter()
                     .map(field::EntityEntry::MIME)
                     .collect();
        let unstr: Vec<header::Unstructured> = arbitrary_vec_where(u, |f: &header::Unstructured| {
            !mime::field::is_mime_header(&f.name)
        })?;
        entries.extend(unstr.into_iter().map(field::EntityEntry::Unstructured));
        arbitrary_shuffle(u, &mut entries)?;
        Ok(AnyPart {
            entries,
            mime_body,
            raw: RawInput::none(),
            raw_headers: RawInput::none(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary, FuzzEq))]
pub enum MimeBody<'a> {
    Mult(Multipart<'a>),
    Msg(Message<'a>),
    Txt(Text<'a>),
    Bin(Binary<'a>),
}
impl<'a> MimeBody<'a> {
    pub fn as_multipart(&self) -> Option<&Multipart<'a>> {
        match self {
            Self::Mult(x) => Some(x),
            _ => None,
        }
    }
    pub fn as_message(&self) -> Option<&Message<'a>> {
        match self {
            Self::Msg(x) => Some(x),
            _ => None,
        }
    }
    pub fn as_text(&self) -> Option<&Text<'a>> {
        match self {
            Self::Txt(x) => Some(x),
            _ => None,
        }
    }
    pub fn as_binary(&self) -> Option<&Binary<'a>> {
        match self {
            Self::Bin(x) => Some(x),
            _ => None,
        }
    }
    pub fn mime(&self) -> AnyMIME<'a> {
        match self {
            Self::Mult(v) => v.mime.clone().into(),
            Self::Msg(v) => v.mime.clone().into(),
            Self::Txt(v) => v.mime.clone().into(),
            Self::Bin(v) => v.mime.clone().into(),
        }
    }
    pub fn raw_body(&self) -> RawInput<'a> {
        match self {
            Self::Mult(v) => v.raw_body.clone(),
            Self::Msg(v) => v.raw_body.clone(),
            Self::Txt(v) => v.raw_body.clone(),
            Self::Bin(v) => v.raw_body.clone(),
        }
    }
    pub fn print_body(&self, fmt: &mut impl Formatter) {
        match &self {
            MimeBody::Mult(multipart) => {
                // TODO: also print preamble and epilogue?
                for child in &multipart.children {
                    fmt.write_bytes(b"--");
                    fmt.write_current_boundary();
                    fmt.write_crlf();
                    child.print(fmt);
                    fmt.write_crlf();
                }
                fmt.write_bytes(b"--");
                fmt.write_current_boundary();
                fmt.write_bytes(b"--");
                fmt.write_crlf();
                fmt.pop_boundary();
            },
            MimeBody::Msg(message) => {
                message.child.print(fmt)
            },
            MimeBody::Txt(text) => {
                fmt.write_bytes(&text.body)
            },
            MimeBody::Bin(binary) => {
                fmt.write_bytes(&binary.body)
            },
        }
    }
}
impl<'a> From<Multipart<'a>> for MimeBody<'a> {
    fn from(m: Multipart<'a>) -> Self {
        Self::Mult(m)
    }
}
impl<'a> From<Message<'a>> for MimeBody<'a> {
    fn from(m: Message<'a>) -> Self {
        Self::Msg(m)
    }
}

impl<'a> Print for AnyPart<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        fmt.begin_line_folding();
        print_seq(fmt, &self.field_list(), |_| ());
        fmt.end_line_folding();
        fmt.write_crlf();
        self.mime_body.print_body(fmt);
    }
}

/// Parse any type of part.
///
/// This function always consumes the whole input.
///
/// ## Note
///
/// Multiparts are a bit special as they have a clearly delimited beginning
/// and end contrary to all the other parts that are going up to the end of the buffer
pub fn part_body<'a>(m: AnyMIME<'a>) -> impl FnOnce(&'a [u8]) -> MimeBody<'a> {
    move |input| {
        let part = match m {
            AnyMIME::Mult(a) => {
                let (_rest, part) = multipart(a)(input);
                #[cfg(feature = "tracing-unsupported")]
                if !_rest.is_empty() {
                    warn!(rest = %bytes_to_trace_string(_rest),
                          "leftover input after multipart parsing")
                }
                part.into()
            },
            AnyMIME::Msg(a) =>
                message(a)(input).into(),
            AnyMIME::Txt(a) => MimeBody::Txt(Text {
                mime: a,
                body: Cow::Borrowed(input),
                raw_body: input.into(),
            }),
            AnyMIME::Bin(a) => MimeBody::Bin(Binary {
                mime: a,
                body: Cow::Borrowed(input),
                raw_body: input.into(),
            }),
        };

        part
    }
}
