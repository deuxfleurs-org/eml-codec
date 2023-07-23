use nom::{
    IResult,
    branch::alt,
    bytes::complete::{is_not},
    multi::many0,
    sequence::{pair},
    combinator::{map, not, recognize},
};

use crate::mime;
use crate::mime::mime::{AnyMIME};
use crate::rfc5322::{self as imf};
use crate::text::boundary::{Delimiter, boundary};
use crate::text::whitespace::obs_crlf;
use crate::text::ascii::CRLF;
use crate::header::{header, CompFieldList};

pub struct Multipart<'a>(pub mime::mime::Multipart<'a>, pub Vec<AnyPart<'a>>);
pub struct Message<'a>(pub mime::mime::Message<'a>, pub imf::message::Message<'a>, pub Box<AnyPart<'a>>);
pub struct Text<'a>(pub mime::mime::Text<'a>, pub &'a [u8]);
pub struct Binary<'a>(pub mime::mime::Binary<'a>, pub &'a [u8]);

pub enum AnyPart<'a> {
    Mult(Multipart<'a>),
    Msg(Message<'a>),
    Txt(Text<'a>),
    Bin(Binary<'a>),
}

pub enum MixedField<'a> {
    MIME(mime::field::Content<'a>),
    IMF(imf::field::Field<'a>),
}
impl<'a> MixedField<'a> {
    pub fn mime(&self) -> Option<&mime::field::Content<'a>> {
        match self {
            Self::MIME(v) => Some(v),
            _ => None,
        }
    }
    pub fn to_mime(self) -> Option<mime::field::Content<'a>> {
        match self {
            Self::MIME(v) => Some(v),
            _ => None,
        }
    }
    pub fn imf(&self) -> Option<&imf::field::Field<'a>> {
        match self {
            Self::IMF(v) => Some(v),
            _ => None,
        }
    }
    pub fn to_imf(self) -> Option<imf::field::Field<'a>> {
        match self {
            Self::IMF(v) => Some(v),
            _ => None,
        }
    }
}
impl<'a> CompFieldList<'a, MixedField<'a>> {
    pub fn sections(self) -> (mime::mime::AnyMIME<'a>, imf::message::Message<'a>) {
        let k = self.known();
        let (v1, v2): (Vec<MixedField>, Vec<MixedField>) = k.into_iter().partition(|v| v.mime().is_some());
        let mime = v1.into_iter().map(|v| v.to_mime()).flatten().collect::<mime::mime::AnyMIME>();
        let imf = v2.into_iter().map(|v| v.to_imf()).flatten().collect::<imf::message::Message>();
        (mime, imf)
    }
}
pub fn mixed_field(input: &[u8]) -> IResult<&[u8], MixedField> {
    alt((
        map(mime::field::content, MixedField::MIME),
        map(imf::field::field, MixedField::IMF),
    ))(input)
}

pub fn message<'a>(m: mime::mime::Message<'a>) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], Message<'a>> {
    move |input: &[u8]| {
        let (input, fields) = header(mixed_field)(input)?;
        let (in_mime, imf) = fields.sections();

        let part = to_anypart(in_mime, input);

        Ok((&[], Message(m.clone(), imf, Box::new(part))))
    }
}

pub fn multipart<'a>(m: mime::mime::Multipart<'a>) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], Multipart<'a>> {
    let m = m.clone();

    move |input| {
        let bound = m.0.boundary.as_bytes();
        let (mut input_loop, _) = part_raw(bound)(input)?;
        let mut mparts: Vec<AnyPart> = vec![];
        loop {
            let input = match boundary(bound)(input_loop) {
                Err(_) => return Ok((input_loop, Multipart(m.clone(), mparts))),
                Ok((inp, Delimiter::Last)) => return Ok((inp, Multipart(m.clone(), mparts))),
                Ok((inp, Delimiter::Next)) => inp,
            };

            // parse mime headers
            let (input, fields) = header(mime::field::content)(input)?;
            let mime = fields.to_mime();

            // parse raw part
            let (input, rpart) = part_raw(bound)(input)?;

            // parse mime body
            mparts.push(to_anypart(mime, rpart));

            input_loop = input;
        }
    }
}

pub fn to_anypart<'a>(m: AnyMIME<'a>, rpart: &'a [u8]) -> AnyPart<'a> {
    match m {
        AnyMIME::Mult(a) => map(multipart(a), AnyPart::Mult)(rpart)
                                .map(|v| v.1)
                                .unwrap_or(AnyPart::Txt(Text(mime::mime::Text::default(), rpart))),
        AnyMIME::Msg(a) => map(message(a), AnyPart::Msg)(rpart)
                                .map(|v| v.1)
                                .unwrap_or(AnyPart::Txt(Text(mime::mime::Text::default(), rpart))),
        AnyMIME::Txt(a) => AnyPart::Txt(Text(a, rpart)),
        AnyMIME::Bin(a) => AnyPart::Bin(Binary(a, rpart)),
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

/*
pub fn preamble<'a>(bound: &'a [u8]) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], &'a [u8]> {
    move |input: &[u8]| {
        recognize(many0(tuple((
            is_not(CRLF), 
            many0(pair(not(boundary(bound)), obs_crlf)),
        ))))(input)
    }
}*/

// FIXME parse email here

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preamble() {
        assert_eq!(
            preamble(b"hello")(b"blip
bloup

blip
bloup--
--bim
--bim--

--hello
Field: Body
"),
            Ok((
                &b"\n--hello\nField: Body\n"[..],
                &b"blip\nbloup\n\nblip\nbloup--\n--bim\n--bim--\n"[..],
            ))
        );
    }

    #[test]
    fn test_part_raw() {
        assert_eq!(
            part(b"simple boundary")(b"Content-type: text/plain; charset=us-ascii

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

    #[test]
    fn test_multipart() {
        assert_eq!(
            multipart(b"simple boundary")(b"This is the preamble.  It is to be ignored, though it
is a handy place for composition agents to include an
explanatory note to non-MIME conformant readers.

--simple boundary

This is implicitly typed plain US-ASCII text.
It does NOT end with a linebreak.
--simple boundary
Content-type: text/plain; charset=us-ascii

This is explicitly typed plain US-ASCII text.
It DOES end with a linebreak.

--simple boundary--

This is the epilogue. It is also to be ignored.
"),
            Ok((&b"\nThis is the epilogue. It is also to be ignored.\n"[..],
                vec![
                    &b"\nThis is implicitly typed plain US-ASCII text.\nIt does NOT end with a linebreak."[..],
                    &b"Content-type: text/plain; charset=us-ascii\n\nThis is explicitly typed plain US-ASCII text.\nIt DOES end with a linebreak.\n"[..],
                ]
            )),
        );
    }
}
