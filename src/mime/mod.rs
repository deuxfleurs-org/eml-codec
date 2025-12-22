/// Parsed and represent an email character set
pub mod charset;

/// MIME specific headers
pub mod field;

/// Transfer-Encoding representation
pub mod mechanism;

/// Content-Type representation
pub mod r#type;

use bounded_static::ToStatic;
use std::fmt;

use crate::header;
use crate::imf::identification::MessageID;
use crate::mime::field::Content;
use crate::mime::mechanism::Mechanism;
use crate::mime::r#type::{AnyType, NaiveType};
use crate::text::misc_token::Unstructured; //Multipart, Message, Text, Binary};

#[derive(Default, PartialEq, Clone, ToStatic)]
pub struct CommonMIME<'a> {
    pub transfer_encoding: Mechanism<'a>,
    pub id: Option<MessageID<'a>>,
    pub description: Option<Unstructured<'a>>,
    // XXX: could `uninterp_headers` be moved to the parent e.g. Message?
    // (to be alongside imf and mime)
    pub uninterp_headers: Vec<header::Unstructured<'a>>,
}
impl<'a> fmt::Debug for CommonMIME<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("CommonMIME")
            .field("transfer_encoding", &self.transfer_encoding)
            .field("id", &self.id)
            .field("description", &self.description)
            .field("uninterp_headers", &self.uninterp_headers)
            .finish()
    }
}

#[derive(Debug, PartialEq, Clone, ToStatic)]
pub struct MIME<'a, T> {
    pub ctype: T,
    pub fields: CommonMIME<'a>,
}

impl<'a> Default for MIME<'a, r#type::DeductibleText> {
    fn default() -> Self {
        Self {
            ctype: r#type::DeductibleText::default(),
            fields: CommonMIME::default(),
        }
    }
}
impl<'a> Default for MIME<'a, r#type::DeductibleMessage> {
    fn default() -> Self {
        Self {
            ctype: r#type::DeductibleMessage::default(),
            fields: CommonMIME::default(),
        }
    }
}

#[derive(Debug, PartialEq, Clone, ToStatic)]
pub enum AnyMIME<'a> {
    Mult(MIME<'a, r#type::Multipart>),
    Msg(MIME<'a, r#type::DeductibleMessage>),
    Txt(MIME<'a, r#type::DeductibleText>),
    Bin(MIME<'a, r#type::Binary>),
}
impl<'a> AnyMIME<'a> {
    pub fn fields(&self) -> &CommonMIME<'a> {
        match self {
            Self::Mult(v) => &v.fields,
            Self::Msg(v) => &v.fields,
            Self::Txt(v) => &v.fields,
            Self::Bin(v) => &v.fields,
        }
    }
}

impl<'a> Into<AnyMIME<'a>> for MIME<'a, r#type::Multipart> {
    fn into(self) -> AnyMIME<'a> {
        AnyMIME::Mult(self)
    }
}

impl<'a> Into<AnyMIME<'a>> for MIME<'a, r#type::DeductibleMessage> {
    fn into(self) -> AnyMIME<'a> {
        AnyMIME::Msg(self)
    }
}

impl<'a> Into<AnyMIME<'a>> for MIME<'a, r#type::DeductibleText> {
    fn into(self) -> AnyMIME<'a> {
        AnyMIME::Txt(self)
    }
}
impl<'a> Into<AnyMIME<'a>> for MIME<'a, r#type::Binary> {
    fn into(self) -> AnyMIME<'a> {
        AnyMIME::Bin(self)
    }
}

// XXX inline and remove alias (by consistency by the other forms of MIME<'a, _> which do not have an alias)
pub type NaiveMIME<'a> = MIME<'a, Option<NaiveType<'a>>>;

impl<'a> Default for NaiveMIME<'a> {
    fn default() -> Self {
        Self {
            ctype: None,
            fields: CommonMIME::default(),
        }
    }
}

impl<'a> FromIterator<Content<'a>> for NaiveMIME<'a> {
    fn from_iter<I: IntoIterator<Item = Content<'a>>>(it: I) -> Self {
        it.into_iter()
            .fold(NaiveMIME::default(), |mut section, field| {
                match field {
                    Content::Type(v) => section.ctype = Some(v),
                    Content::TransferEncoding(v) => section.fields.transfer_encoding = v,
                    Content::ID(v) => section.fields.id = Some(v),
                    Content::Description(v) => section.fields.description = Some(v),
                };
                section
            })
    }
}

impl<'a> NaiveMIME<'a> {
    pub fn to_interpreted(self, default_type: DefaultType) -> AnyMIME<'a> {
        let typ: AnyType = self
            .ctype
            .as_ref()
            .map(NaiveType::to_type)
            .unwrap_or(default_type.to_type());

        match typ {
            AnyType::Multipart(ctype) => AnyMIME::Mult(MIME {
                ctype,
                fields: self.fields,
            }),
            AnyType::Message(ctype) => AnyMIME::Msg(MIME {
                ctype,
                fields: self.fields,
            }),
            AnyType::Text(ctype) => AnyMIME::Txt(MIME {
                ctype,
                fields: self.fields,
            }),
            AnyType::Binary(ctype) => AnyMIME::Bin(MIME {
                ctype,
                fields: self.fields,
            }),
        }
    }
}

#[derive(Default)]
pub enum DefaultType {
    #[default]
    Generic,
    Digest,
}

impl DefaultType {
    fn to_type(self) -> AnyType {
        match self {
            Self::Generic => AnyType::Text(Default::default()),
            Self::Digest => AnyType::Message(Default::default()),
        }
    }
}
