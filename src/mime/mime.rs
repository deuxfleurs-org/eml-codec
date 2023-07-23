use crate::mime::mechanism::Mechanism;
use crate::rfc5322::identification::MessageID;
use crate::text::misc_token::Unstructured;
use crate::mime::field::Content;
use crate::mime::r#type::{AnyType, self as ctype}; //Multipart, Message, Text, Binary};
 
#[derive(Debug, PartialEq, Clone)]
pub struct Multipart<'a>(pub ctype::Multipart, pub Generic<'a>);

#[derive(Debug, PartialEq, Clone)]
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

impl<'a> FromIterator<Content<'a>> for AnyMIME<'a> {
    fn from_iter<I: IntoIterator<Item = Content<'a>>>(it: I) -> Self {
         let (at, gen) = it.into_iter().fold(
            (AnyType::default(), Generic::default()),
            |(mut at, mut section), field| {
                match field {
                    Content::Type(v) => at = v.to_type(),
                    Content::TransferEncoding(v) => section.transfer_encoding = v,
                    Content::ID(v) => section.id = Some(v),
                    Content::Description(v) => section.description = Some(v),
                };
                (at, section)
            }
        );
       
        Self::from_pair(at, gen)
    }
}

#[derive(Debug, PartialEq, Default, Clone)]
pub struct Generic<'a> {
    pub transfer_encoding: Mechanism<'a>,
    pub id: Option<MessageID<'a>>,
    pub description: Option<Unstructured<'a>>,
}

