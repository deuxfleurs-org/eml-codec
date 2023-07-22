use std::borrow::Cow;
use encoding_rs::Encoding;
use nom::{
    branch::alt,
    bytes::complete::{tag,take_while1}, 
    character::complete as character, 
    combinator::{into, opt}, 
    multi::many0,
    sequence::{delimited, preceded, tuple},
    IResult,
};

use crate::text::whitespace::cfws;
use crate::text::quoted::quoted_string;
use crate::text::misc_token::{MIMEWord, mime_word};
use crate::text::words::{mime_atom};

// --------- NAIVE TYPE
#[derive(Debug, PartialEq)]
pub struct NaiveType<'a> {
    main: MIMEWord<'a>,
    sub: MIMEWord<'a>,
    params: Parameter<'a>,
}
impl<'a> NaiveType<'a> {
    pub fn decode(&self) -> Type<'a> {
        Type::from_naive_type(self)
    } 
}
pub fn naive_type(input: &[u8]) -> IResult<&[u8], Type> {
    map(
        tuple((mime_atom, tag("/"), mime_atom, parameter_list)),
        |(main, _, sub, params)| Type { main, sub, params },
    )(input)
}

#[derive(Debug, PartialEq)]
pub enum Parameter<'a> {
    name: &'a [u8],
    value: MIMEWord<'a>,
}
pub fn parameter(input: &[u8]) -> IResult<&[u8], Parameter> {
    map(tuple((mime_atom, tag(b"="), mime_word)), |(name, value)| Parameter { name, value })(input)
}
pub fn parameter_list(input: &[u8]) -> IResult<&[u8], Vec<Parameter>> {
    many0(preceded(tag(";"), parameter))(input)
}

// -------- TYPE
#[derive(Debug, PartialEq, Default)]
pub enum Type<'a> {
    // Composite types
    Multipart(MultipartDesc<'a>),
    Message(MessageSubtype<'a>),

    // Discrete types
    Text(TextDesc<'a>),
    Binary,
}
impl<'a> Type<'a> {
    pub fn from_naive_type(nt: &NaiveType<'a>) -> Self {
        match nt.main.to_ascii_lowercase().as_slice() {
            b"multipart" => MultipartDesc::from_naive_type(nt).map(Self::Multipart).unwrap_or(Self::default()),
            b"message" => Self::Message(MessageDesc::from_naive_type(nt)),
            b"text" => Self::Text(TextDesc::from_naive_type(nt)),
            _ => Self::Binary,
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct MultipartDesc<'a> {
    pub subtype: MultipartSubtype<'a>,
    pub boundary: &'a [u8],
}
impl<'a> MultipartDesc<'a> {
    pub fn from_naive_type(nt: &NaiveType<'a>) -> Option<Self> {
        MultipartDesc {
            subtype: MultipartSubtype::from_naive_type(nt),
            boundary: nt.iter().find(|x| x.name.as_ascii_lowercase().as_slice() == b"boundary").unwrap_or(&[]),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum MultipartSubtype {
    Alternative,
    Mixed,
    Digest,
    Parallel,
    Report,
    Unknown,
}
impl<'a> From<&NaiveType<'a>> for MultipartSubtype<'a> {
    pub fn from(nt: &NaiveType<'a>) -> Self {
        match nt.sub.as_ascii_lowercase().as_slice() {
            b"alternative" => Self::Alternative,
            b"mixed" => Self::Mixed,
            b"digest" => Self::Digest,
            b"parallel" => Self::Parallel,
            b"report" => Self::Report,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum MessageSubtype {
    RFC822,
    Partial,
    External,
    Unknown,
}
impl<'a> From<&NaiveType<'a>> for MessageSubtype {
    fn from(nt: &NaiveType<'a>) -> Self {
        match csub.to_lowercase().as_ref() {
            "rfc822" => MessageSubtype::RFC822,
            "partial" => MessageSubtype::Partial,
            "external" => MessageSubtype::External,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, PartialEq, Default)]
pub struct TextDesc<'a> {
    pub charset: Option<EmailCharset<'a>>,
    pub subtype: TextSubtype<'a>,
}

#[derive(Debug, PartialEq, Default)]
pub enum TextSubtype<'a> {
    Plain,
    Html,
    Other(&'a str),
}
