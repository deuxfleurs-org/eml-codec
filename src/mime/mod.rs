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

use crate::imf::identification::MessageID;
use crate::mime::field::Content;
use crate::mime::mechanism::Mechanism;
use crate::mime::r#type::{AnyType, NaiveType};
use crate::text::misc_token::Unstructured; //Multipart, Message, Text, Binary};
use crate::utils::set_opt;

#[derive(Default, PartialEq, Clone, ToStatic)]
pub struct CommonMIME<'a> {
    pub transfer_encoding: Mechanism<'a>,
    pub id: Option<MessageID<'a>>,
    pub description: Option<Unstructured<'a>>,
}
impl<'a> fmt::Debug for CommonMIME<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("CommonMIME")
            .field("transfer_encoding", &self.transfer_encoding)
            .field("id", &self.id)
            .field("description", &self.description)
            .finish()
    }
}

#[derive(Debug, PartialEq, Clone, ToStatic)]
pub struct MIME<'a, T> {
    pub ctype: T,
    pub fields: CommonMIME<'a>,
}

impl<'a> Default for MIME<'a, r#type::DeductibleText<'a>> {
    fn default() -> Self {
        Self {
            ctype: r#type::DeductibleText::default(),
            fields: CommonMIME::default(),
        }
    }
}
impl<'a> Default for MIME<'a, r#type::DeductibleMessage<'a>> {
    fn default() -> Self {
        Self {
            ctype: r#type::DeductibleMessage::default(),
            fields: CommonMIME::default(),
        }
    }
}

#[derive(Debug, PartialEq, Clone, ToStatic)]
pub enum AnyMIME<'a> {
    Mult(MIME<'a, r#type::Multipart<'a>>),
    Msg(MIME<'a, r#type::DeductibleMessage<'a>>),
    Txt(MIME<'a, r#type::DeductibleText<'a>>),
    Bin(MIME<'a, r#type::Binary<'a>>),
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

impl<'a> Into<AnyMIME<'a>> for MIME<'a, r#type::Multipart<'a>> {
    fn into(self) -> AnyMIME<'a> {
        AnyMIME::Mult(self)
    }
}

impl<'a> Into<AnyMIME<'a>> for MIME<'a, r#type::DeductibleMessage<'a>> {
    fn into(self) -> AnyMIME<'a> {
        AnyMIME::Msg(self)
    }
}

impl<'a> Into<AnyMIME<'a>> for MIME<'a, r#type::DeductibleText<'a>> {
    fn into(self) -> AnyMIME<'a> {
        AnyMIME::Txt(self)
    }
}
impl<'a> Into<AnyMIME<'a>> for MIME<'a, r#type::Binary<'a>> {
    fn into(self) -> AnyMIME<'a> {
        AnyMIME::Bin(self)
    }
}

#[derive(Clone, Debug, Default, PartialEq, ToStatic)]
pub struct NaiveMIME<'a> {
    ctype: Option<r#type::NaiveType<'a>>,
    transfer_encoding: Option<Mechanism<'a>>,
    id: Option<MessageID<'a>>,
    description: Option<Unstructured<'a>>,
}

#[derive(Clone, Copy, Debug, PartialEq, ToStatic)]
pub enum FieldEntry {
    Type,
    TransferEncoding,
    ID,
    Description,
}

impl<'a> NaiveMIME<'a> {
    pub fn add_field(&mut self, f: Content<'a>) -> Option<FieldEntry> {
        match f {
            Content::Type(ctype) =>
                set_opt(&mut self.ctype, ctype).then_some(FieldEntry::Type),
            Content::TransferEncoding(enc) =>
                set_opt(&mut self.transfer_encoding, enc).then_some(FieldEntry::TransferEncoding),
            Content::ID(id) =>
                set_opt(&mut self.id, id).then_some(FieldEntry::ID),
            Content::Description(desc) =>
                set_opt(&mut self.description, desc).then_some(FieldEntry::Description),
        }
    }

    pub fn to_interpreted(self, default_type: DefaultType) -> AnyMIME<'a> {
        let typ: AnyType = self
            .ctype
            .as_ref()
            .map(NaiveType::to_type)
            .unwrap_or(default_type.to_type());
        let mut fields = CommonMIME {
            transfer_encoding: self.transfer_encoding.unwrap_or_default(),
            id: self.id,
            description: self.description,
        };

        match typ {
            AnyType::Multipart(ctype) => {
                // Ensure we are using an encoding allowed for multipart
                fields.transfer_encoding = fields.transfer_encoding.to_part_encoding();
                AnyMIME::Mult(MIME { ctype, fields })
            },
            AnyType::Message(ctype) => {
                // Ensure we are using an encoding allowed for message/rfc822
                // TODO: enforce corresponding restrictions for other message subtypes
                fields.transfer_encoding = fields.transfer_encoding.to_part_encoding();
                AnyMIME::Msg(MIME { ctype, fields })
            },
            AnyType::Text(ctype) => AnyMIME::Txt(MIME { ctype, fields }),
            AnyType::Binary(ctype) => AnyMIME::Bin(MIME { ctype, fields }),
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
    fn to_type(self) -> AnyType<'static> {
        match self {
            Self::Generic => AnyType::Text(Default::default()),
            Self::Digest => AnyType::Message(Default::default()),
        }
    }
}
