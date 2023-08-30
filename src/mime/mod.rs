/// Parsed and represent an email character set
pub mod charset;

/// MIME specific headers
pub mod field;

/// Transfer-Encoding representation
pub mod mechanism;

/// Content-Type representation
pub mod r#type;

use std::fmt;
use std::marker::PhantomData;

use crate::header;
use crate::imf::identification::MessageID;
use crate::mime::field::Content;
use crate::mime::mechanism::Mechanism;
use crate::mime::r#type::{AnyType, NaiveType};
use crate::text::misc_token::Unstructured; //Multipart, Message, Text, Binary};

#[derive(Debug, PartialEq, Clone)]
pub struct MIME<'a, T> {
    pub interpreted_type: T,
    pub fields: NaiveMIME<'a>,
}
impl<'a> Default for MIME<'a, r#type::DeductibleText> {
    fn default() -> Self {
        Self {
            interpreted_type: r#type::DeductibleText::default(),
            fields: NaiveMIME::default(),
        }
    }
}
impl<'a> Default for MIME<'a, r#type::DeductibleMessage> {
    fn default() -> Self {
        Self {
            interpreted_type: r#type::DeductibleMessage::default(),
            fields: NaiveMIME::default(),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum AnyMIME<'a> {
    Mult(MIME<'a, r#type::Multipart>),
    Msg(MIME<'a, r#type::DeductibleMessage>),
    Txt(MIME<'a, r#type::DeductibleText>),
    Bin(MIME<'a, r#type::Binary>),
}

impl<'a, T: WithDefaultType> From<AnyMIMEWithDefault<'a, T>> for AnyMIME<'a> {
    fn from(a: AnyMIMEWithDefault<'a, T>) -> Self {
        a.0
    }
}

#[derive(PartialEq, Default, Clone)]
pub struct NaiveMIME<'a> {
    pub ctype: Option<NaiveType<'a>>,
    pub transfer_encoding: Mechanism<'a>,
    pub id: Option<MessageID<'a>>,
    pub description: Option<Unstructured<'a>>,
    pub kv: Vec<header::Field<'a>>,
    pub raw: &'a [u8],
}
impl<'a> fmt::Debug for NaiveMIME<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("NaiveMime")
            .field("ctype", &self.ctype)
            .field("transfer_encoding", &self.transfer_encoding)
            .field("id", &self.id)
            .field("description", &self.description)
            .field("kv", &self.kv)
            .field("raw", &String::from_utf8_lossy(self.raw))
            .finish()
    }
}

impl<'a> FromIterator<Content<'a>> for NaiveMIME<'a> {
    fn from_iter<I: IntoIterator<Item = Content<'a>>>(it: I) -> Self {
        it.into_iter()
            .fold(NaiveMIME::default(), |mut section, field| {
                match field {
                    Content::Type(v) => section.ctype = Some(v),
                    Content::TransferEncoding(v) => section.transfer_encoding = v,
                    Content::ID(v) => section.id = Some(v),
                    Content::Description(v) => section.description = Some(v),
                };
                section
            })
    }
}

impl<'a> NaiveMIME<'a> {
    pub fn with_kv(mut self, fields: Vec<header::Field<'a>>) -> Self {
        self.kv = fields;
        self
    }
    pub fn with_raw(mut self, raw: &'a [u8]) -> Self {
        self.raw = raw;
        self
    }
    pub fn to_interpreted<T: WithDefaultType>(self) -> AnyMIME<'a> {
        self.ctype
            .as_ref()
            .map(|c| c.to_type())
            .unwrap_or(T::default_type())
            .to_mime(self)
            .into()
    }
}

pub trait WithDefaultType {
    fn default_type() -> AnyType;
}

pub struct WithGenericDefault {}
impl WithDefaultType for WithGenericDefault {
    fn default_type() -> AnyType {
        AnyType::Text(r#type::DeductibleText::default())
    }
}
pub struct WithDigestDefault {}
impl WithDefaultType for WithDigestDefault {
    fn default_type() -> AnyType {
        AnyType::Message(r#type::DeductibleMessage::default())
    }
}

#[derive(Debug, PartialEq)]
pub struct AnyMIMEWithDefault<'a, T: WithDefaultType>(pub AnyMIME<'a>, PhantomData<T>);

impl<'a, T: WithDefaultType> Default for AnyMIMEWithDefault<'a, T> {
    fn default() -> Self {
        Self(T::default_type().to_mime(NaiveMIME::default()), PhantomData)
    }
}
