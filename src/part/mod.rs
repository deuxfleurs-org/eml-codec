/// Parts that contain other parts inside them
pub mod composite;

/// Parts that have a body and no child parts
pub mod discrete;

/// Representation of all headers in a MIME entity
pub mod field;

use bounded_static::ToStatic;
use nom::{
    branch::alt,
    bytes::complete::is_not,
    combinator::{not, recognize},
    multi::many0,
    sequence::pair,
    IResult,
};
use std::borrow::Cow;

use crate::mime;
use crate::mime::AnyMIME;
use crate::part::{
    composite::{message, multipart, Message, Multipart},
    discrete::{Binary, Text},
};
use crate::print::{Print, Formatter};
use crate::text::ascii::CRLF;
use crate::text::boundary::boundary;
use crate::text::whitespace::obs_crlf;

#[derive(Clone, Debug, PartialEq, ToStatic)]
pub struct AnyPart<'a> {
    // Invariant: `fields` must be "complete and correct":
    // - it must contain an entry for every piece of information contained in
    //   `mime_body`'s mime headers that is not the default value. (This means
    //    values of which are `Deductible::Explicit` or optionals set to
    //    `Some(_)`.)
    // - it must *only* contain entries for fields that have a value. (This means
    //   no optional fields set to `None`.)
    // Invariant: `fields` must contain no duplicates.
    pub fields: Vec<field::EntityField<'a>>,
    pub mime_body: MimeBody<'a>,
}

#[derive(Clone, Debug, PartialEq, ToStatic)]
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
        let mime = self.mime_body.mime();
        for field in &self.fields {
            match field {
                field::EntityField::Unstructured(u) => u.print(fmt),
                field::EntityField::MIME(f) => mime.print_field(*f, fmt),
            }
        }
        fmt.end_line_folding();
        fmt.write_crlf();
        self.mime_body.print_body(fmt);
    }
}

/// Parse any type of part
///
/// ## Note
///
/// Multiparts are a bit special as they have a clearly delimited beginning
/// and end contrary to all the other parts that are going up to the end of the buffer
pub fn part_body<'a>(m: AnyMIME<'a>) -> impl FnOnce(&'a [u8]) -> IResult<&'a [u8], MimeBody<'a>> {
    move |input| {
        let part = match m {
            AnyMIME::Mult(a) => multipart(a)(input)
                .map(|(_, multi)| multi.into())
                .unwrap_or(MimeBody::Txt(Text {
                    mime: mime::MIME::<mime::r#type::DeductibleText>::default(),
                    body: Cow::Borrowed(input),
                })),
            AnyMIME::Msg(a) => {
                message(a)(input)
                    .map(|(_, msg)| msg.into())
                    .unwrap_or(MimeBody::Txt(Text {
                        mime: mime::MIME::<mime::r#type::DeductibleText>::default(),
                        body: Cow::Borrowed(input),
                    }))
            }
            AnyMIME::Txt(a) => MimeBody::Txt(Text {
                mime: a,
                body: Cow::Borrowed(input),
            }),
            AnyMIME::Bin(a) => MimeBody::Bin(Binary {
                mime: a,
                body: Cow::Borrowed(input),
            }),
        };

        // This function always consumes the whole input
        Ok((&input[input.len()..], part))
    }
}

pub fn part_raw<'a>(bound: &[u8]) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], &'a [u8]> + '_ {
    move |input| {
        // XXX could this parser be defined in a way that matches the spec more naturally?
        recognize(many0(pair(
            not(boundary(bound)),
            alt((is_not(CRLF), obs_crlf)),
        )))(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preamble() {
        assert_eq!(
            part_raw(b"hello")(
                b"blip
bloup

blip
bloup--
--bim
--bim--

--hello
Field: Body
"
            ),
            Ok((
                &b"\n--hello\nField: Body\n"[..],
                &b"blip\nbloup\n\nblip\nbloup--\n--bim\n--bim--\n"[..],
            ))
        );
    }

    #[test]
    fn test_part_raw() {
        assert_eq!(
            part_raw(b"simple boundary")(b"Content-type: text/plain; charset=us-ascii

This is explicitly typed plain US-ASCII text.
It DOES end with a linebreak.

--simple boundary--
"),
            Ok((
                &b"\n--simple boundary--\n"[..], 
                &b"Content-type: text/plain; charset=us-ascii\n\nThis is explicitly typed plain US-ASCII text.\nIt DOES end with a linebreak.\n"[..],
            ))
        );
    }
}
