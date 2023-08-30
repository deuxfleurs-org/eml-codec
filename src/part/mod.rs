/// Parts that contain other parts inside them
pub mod composite;

/// Parts that have a body and no child parts
pub mod discrete;

/// IMF + MIME fields parsed at once
pub mod field;

use nom::{
    branch::alt,
    bytes::complete::is_not,
    combinator::{not, recognize},
    multi::many0,
    sequence::pair,
    IResult,
};

use crate::mime;
use crate::mime::AnyMIME;
use crate::part::{
    composite::{message, multipart, Message, Multipart},
    discrete::{Binary, Text},
};
use crate::text::ascii::CRLF;
use crate::text::boundary::boundary;
use crate::text::whitespace::obs_crlf;

#[derive(Debug, PartialEq)]
pub enum AnyPart<'a> {
    Mult(Multipart<'a>),
    Msg(Message<'a>),
    Txt(Text<'a>),
    Bin(Binary<'a>),
}
impl<'a> AnyPart<'a> {
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
}
impl<'a> From<Multipart<'a>> for AnyPart<'a> {
    fn from(m: Multipart<'a>) -> Self {
        Self::Mult(m)
    }
}
impl<'a> From<Message<'a>> for AnyPart<'a> {
    fn from(m: Message<'a>) -> Self {
        Self::Msg(m)
    }
}

/// Parse any type of part
///
/// ## Note
///
/// Multiparts are a bit special as they have a clearly delimited beginning
/// and end contrary to all the other parts that are going up to the end of the buffer
pub fn anypart<'a>(m: AnyMIME<'a>) -> impl FnOnce(&'a [u8]) -> IResult<&'a [u8], AnyPart<'a>> {
    move |input| {
        let part = match m {
            AnyMIME::Mult(a) => multipart(a)(input)
                .map(|(_, multi)| multi.into())
                .unwrap_or(AnyPart::Txt(Text {
                    mime: mime::MIME::<mime::r#type::DeductibleText>::default(),
                    body: input,
                })),
            AnyMIME::Msg(a) => {
                message(a)(input)
                    .map(|(_, msg)| msg.into())
                    .unwrap_or(AnyPart::Txt(Text {
                        mime: mime::MIME::<mime::r#type::DeductibleText>::default(),
                        body: input,
                    }))
            }
            AnyMIME::Txt(a) => AnyPart::Txt(Text {
                mime: a,
                body: input,
            }),
            AnyMIME::Bin(a) => AnyPart::Bin(Binary {
                mime: a,
                body: input,
            }),
        };

        // This function always consumes the whole input
        Ok((&input[input.len()..], part))
    }
}

pub fn part_raw<'a>(bound: &[u8]) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], &'a [u8]> + '_ {
    move |input| {
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
