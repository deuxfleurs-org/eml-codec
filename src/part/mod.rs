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

pub fn to_anypart<'a>(m: AnyMIME<'a>, rpart: &'a [u8]) -> AnyPart<'a> {
    match m {
        AnyMIME::Mult(a) => multipart(a)(rpart)
            .map(|(rest, multi)| AnyPart::Mult(multi.with_epilogue(rest)))
            .unwrap_or(AnyPart::Txt(Text {
                interpreted: mime::Text::default(),
                body: rpart,
            })),
        AnyMIME::Msg(a) => message(a)(rpart)
            .map(|(rest, msg)| AnyPart::Msg(msg.with_epilogue(rest)))
            .unwrap_or(AnyPart::Txt(Text {
                interpreted: mime::Text::default(),
                body: rpart,
            })),
        AnyMIME::Txt(a) => AnyPart::Txt(Text {
            interpreted: a,
            body: rpart,
        }),
        AnyMIME::Bin(a) => AnyPart::Bin(Binary {
            interpreted: a,
            body: rpart,
        }),
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
