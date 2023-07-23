use nom::{
    IResult,
    branch::alt,
    bytes::complete::{is_not, tag},
    multi::many0,
    sequence::{pair, preceded, tuple},
    combinator::{not, opt, recognize},
};

use crate::mime::r#type as ctype;
use crate::mime::mime;
use crate::rfc5322::{self as imf};

pub struct Multipart(pub mime::Multipart, pub Vec<Part<'a>>);
pub struct Message(pub mime::Message, pub imf::message::Message, pub Part<'a>);
pub struct Text(pub mime::Text, pub &'a [u8]);
pub struct Binary(pub mime::Binary, pub &'a [u8]);

pub struct AnyPart<'a> {
    Mult(Multipart<'a>),
    Msg(Message<'a>),
    Txt(Text<'a>),
    Bin(Binary<'a>),
}

pub enum MixedField<'a> {
    MIME(mime::fields::Content<'a>),
    IMF(rfc5322::fields::Field<'a>),
}
impl<'a> MixedField<'a> {
    pub fn mime(&self) -> Option<&mime::fields::Content<'a>> {
        match self {
            MIME(v) => Some(v),
            _ => None,
        }
    }
    pub fn imf(&self) -> Option<&rfc5322::fields::Field<'a>> {
        match self {
            IMF(v) => Some(v),
            _ => None,
        }
    }
}
impl<'a, MixedField> CompFieldList<'a, MixedField> {
    pub fn sections(self) -> (mime::mime::AnyMIME<'a>, imf::message::Message<'a>) {
        let k = self.known();
        let mime = k.iter().map(MixedField::mime).flatten().collect::<mime::mime::AnyMIME>();
        let imf = k.iter().map(MixedField::imf).flatten().collect::<imf::message::Message>();
        (mime, imf)
    }
}
pub fn mixed_field(input: &[u8]) -> IResult<&[u8], MixedField> {
    alt((
        map(mime::fields::content, MixedField::MIME),
        map(rfc5322::fields::field, MixedField::IMF),
    ))(input)
}

pub fn message<'a>(m: mime::Message<'a>) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], Message<'a>> {
    move |input: &[u8]| {
        let (input, fields) = header(mixed_field)(input)?;
        let (in_mime, imf) = fields.sections();

        let part = to_anypart(in_mime, input);

        Ok((&b[], Message(m, imf, part)))
    }
}

pub fn multipart<'a>(m: mime::Multipart<'a>) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], Multipart<'a>> {
    move |input: &[u8]| {
        let (mut input_loop, _) = part_raw(m.ctype.boundary)(input)?;
        let mut mparts: Vec<AnyPart> = vec![];
        loop {
            let input = match boundary(m.ctype.boundary)(input_loop) {
                Err(_) => return Ok((input_loop, Multipart(m, mparts))),
                Ok((inp, Delimiter::Last)) => return Ok((inp, Multipart(m, mparts))),
                Ok((inp, Delimiter::Next)) => inp,
            };

            // parse mime headers
            let (input, fields) = header(content)(input)?;
            let mime = fields.to_mime();

            // parse raw part
            let (input, rpart) = part_raw(ctype.boundary.as_bytes())(input)?;

            // parse mime body
            mparts.push(to_anypart(mime, rpart);

            input_loop = input;
        }
    }
}

pub fn to_anypart(m: AnyMIME<'a>, rpart: &[u8]) -> AnyPart<'a> {
    match mime {
        AnyMIME::Mult(a) => map(multipart(a), AnyPart::Mult)(rpart)
                                .unwrap_or(AnyPart::Text(Text::default(), rpart)),
        AnyMIME::Msg(a) => map(message(a), AnyPart::Msg)(rpart)
                                .unwrap_or(AnyPart::Text(Text::default(), rpart)),
        AnyMIME::Txt(a) => AnyPart::Txt(Text(a, rpart)),
        AnyMIME::Bin(a) => AnyPart::Bin(Binary(a, rpart)),
    }
}


pub fn part_raw<'a>(bound: &'a [u8]) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], &'a [u8]> {
    move |input: &[u8]| {
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
