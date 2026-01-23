/// MIME specific headers
pub mod field;

/// Transfer-Encoding representation
pub mod mechanism;

/// Content-Type representation
pub mod r#type;

#[cfg(feature = "arbitrary")]
use arbitrary::Arbitrary;
use bounded_static::ToStatic;
use std::collections::HashSet;

#[cfg(feature = "arbitrary")]
use crate::fuzz_eq::FuzzEq;
use crate::header;
use crate::imf::identification::MessageID;
use crate::mime::field::{Content, Entry as FieldEntry};
use crate::mime::mechanism::Mechanism;
use crate::mime::r#type::{AnyType, NaiveType};
use crate::print::Formatter;
use crate::text::misc_token::Unstructured;
use crate::utils::{Deductible, set_opt};

#[derive(Debug, Default, PartialEq, Clone, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary, FuzzEq))]
pub struct CommonMIME<'a> {
    pub transfer_encoding: Deductible<Mechanism<'a>>,
    pub id: Option<MessageID<'a>>,
    pub description: Option<Unstructured<'a>>,
}

#[derive(Debug, PartialEq, Clone, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary, FuzzEq))]
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

#[derive(Debug, PartialEq, Clone, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary, FuzzEq))]
pub enum AnyMIME<'a> {
    Mult(MIME<'a, r#type::Multipart<'a>>),
    Msg(MIME<'a, r#type::Message<'a>>),
    Txt(MIME<'a, r#type::DeductibleText<'a>>),
    Bin(MIME<'a, r#type::Binary<'a>>),
}
impl<'a> AnyMIME<'a> {
    pub fn ctype(&self) -> AnyType<'a> {
        match self {
            Self::Mult(m) => AnyType::Multipart(m.ctype.clone()),
            Self::Msg(m) => AnyType::Message(m.ctype.clone()),
            Self::Txt(m) => AnyType::Text(m.ctype.clone()),
            Self::Bin(m) => AnyType::Binary(m.ctype.clone()),
        }
    }

    pub fn common(&self) -> &CommonMIME<'a> {
        match self {
            Self::Mult(v) => &v.fields,
            Self::Msg(v) => &v.fields,
            Self::Txt(v) => &v.fields,
            Self::Bin(v) => &v.fields,
        }
    }

    pub fn print_field(&self, f: FieldEntry, fmt: &mut impl Formatter) {
        match f {
            FieldEntry::Type =>
                header::print(fmt, b"Content-Type", self.ctype()),
            FieldEntry::TransferEncoding =>
                header::print(
                    fmt,
                    b"Content-Transfer-Encoding",
                    &self.common().transfer_encoding
                ),
            FieldEntry::ID => {
                if let Some(id) = &self.common().id {
                    header::print(fmt, b"Content-Id", id)
                }
            },
            FieldEntry::Description => {
                if let Some(desc) = &self.common().description {
                    header::print(fmt, b"Content-Description", desc)
                }
            },
        }
    }

    pub fn field_entries(&self) -> HashSet<field::Entry> {
        let mut fs = HashSet::default();
        if !matches!(self.ctype(), AnyType::Text(Deductible::Inferred)) {
            fs.insert(field::Entry::Type);
        }
        let common = self.common();
        if let Deductible::Explicit(_) = common.transfer_encoding {
            fs.insert(field::Entry::TransferEncoding);
        }
        if common.id.is_some() {
            fs.insert(field::Entry::ID);
        }
        if common.description.is_some() {
            fs.insert(field::Entry::Description);
        }
        fs
    }
}

impl<'a> Into<AnyMIME<'a>> for MIME<'a, r#type::Multipart<'a>> {
    fn into(self) -> AnyMIME<'a> {
        AnyMIME::Mult(self)
    }
}

impl<'a> Into<AnyMIME<'a>> for MIME<'a, r#type::Message<'a>> {
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
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub struct NaiveMIME<'a> {
    ctype: Option<r#type::NaiveType<'a>>,
    transfer_encoding: Option<Mechanism<'a>>,
    id: Option<MessageID<'a>>,
    description: Option<Unstructured<'a>>,
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
        let transfer_encoding = self.transfer_encoding
            .map(Deductible::Explicit)
            .unwrap_or_default();
        let mut fields = CommonMIME {
            transfer_encoding,
            id: self.id,
            description: self.description,
        };
        match typ {
            AnyType::Multipart(ctype) => {
                // Ensure we are using an encoding allowed for multipart
                fields.transfer_encoding =
                    match fields.transfer_encoding {
                        t @ Deductible::Inferred => t,
                        Deductible::Explicit(t) => Deductible::Explicit(t.to_part_encoding()),
                    };
                AnyMIME::Mult(MIME { ctype, fields })
            },
            AnyType::Message(ctype) => {
                // Ensure we are using an encoding allowed for message/rfc822
                // TODO: enforce corresponding restrictions for other message subtypes
                fields.transfer_encoding =
                    match fields.transfer_encoding {
                        t @ Deductible::Inferred => t,
                        Deductible::Explicit(t) => Deductible::Explicit(t.to_part_encoding()),
                    };
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
