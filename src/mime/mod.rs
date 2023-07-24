/// Parsed and represent an email character set 
pub mod charset;

/// MIME specific headers
pub mod field;

/// Transfer-Encoding representation
pub mod mechanism;

/// Content-Type representation
pub mod r#type;

use std::marker::PhantomData;

use crate::imf::identification::MessageID;
use crate::mime::field::Content;
use crate::mime::mechanism::Mechanism;
use crate::mime::r#type::{self as ctype, AnyType};
use crate::text::misc_token::Unstructured; //Multipart, Message, Text, Binary};

#[derive(Debug, PartialEq, Clone)]
pub struct Multipart<'a>(pub ctype::Multipart, pub Generic<'a>);

#[derive(Debug, PartialEq, Clone, Default)]
pub struct Message<'a>(pub ctype::Message, pub Generic<'a>);

#[derive(Debug, PartialEq, Clone, Default)]
pub struct Text<'a>(pub ctype::Text, pub Generic<'a>);

#[derive(Debug, PartialEq, Clone)]
pub struct Binary<'a>(pub ctype::Binary, pub Generic<'a>);

#[derive(Debug, PartialEq, Clone)]
pub enum AnyMIME<'a> {
    Mult(Multipart<'a>),
    Msg(Message<'a>),
    Txt(Text<'a>),
    Bin(Binary<'a>),
}

impl<'a> AnyMIME<'a> {
    pub fn from_pair(at: AnyType, gen: Generic<'a>) -> Self {
        match at {
            AnyType::Multipart(m) => AnyMIME::Mult(Multipart(m, gen)),
            AnyType::Message(m) => AnyMIME::Msg(Message(m, gen)),
            AnyType::Text(m) => AnyMIME::Txt(Text(m, gen)),
            AnyType::Binary(m) => AnyMIME::Bin(Binary(m, gen)),
        }
    }
}

impl<'a, T: WithDefaultType> From<AnyMIMEWithDefault<'a, T>> for AnyMIME<'a> {
    fn from(a: AnyMIMEWithDefault<'a, T>) -> Self {
        a.0
    }
}

#[derive(Debug, PartialEq, Default, Clone)]
pub struct Generic<'a> {
    pub transfer_encoding: Mechanism<'a>,
    pub id: Option<MessageID<'a>>,
    pub description: Option<Unstructured<'a>>,
}

pub trait WithDefaultType {
    fn default_type() -> AnyType;
}

pub struct WithGenericDefault {}
impl WithDefaultType for WithGenericDefault {
    fn default_type() -> AnyType {
        AnyType::Text(r#type::Text::default())
    }
}
pub struct WithDigestDefault {}
impl WithDefaultType for WithDigestDefault {
    fn default_type() -> AnyType {
        AnyType::Message(r#type::Message::default())
    }
}

#[derive(Debug, PartialEq)]
pub struct AnyMIMEWithDefault<'a, T: WithDefaultType>(pub AnyMIME<'a>, PhantomData<T>);

impl<'a, T: WithDefaultType> FromIterator<Content<'a>> for AnyMIMEWithDefault<'a, T> {
    fn from_iter<I: IntoIterator<Item = Content<'a>>>(it: I) -> Self {
        let (at, gen) = it.into_iter().fold(
            (T::default_type(), Generic::default()),
            |(mut at, mut section), field| {
                match field {
                    Content::Type(v) => at = v.to_type(),
                    Content::TransferEncoding(v) => section.transfer_encoding = v,
                    Content::ID(v) => section.id = Some(v),
                    Content::Description(v) => section.description = Some(v),
                };
                (at, section)
            },
        );

        Self(AnyMIME::from_pair(at, gen), PhantomData)
    }
}
