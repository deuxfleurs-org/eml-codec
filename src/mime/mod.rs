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
use crate::i18n::ContainsUtf8;
use crate::imf::identification::MessageID;
use crate::mime::field::NaiveField;
use crate::mime::mechanism::Mechanism;
use crate::mime::r#type::{AnyType, MessageSubtype, NaiveType};
use crate::text::misc_token::Unstructured;
use crate::utils::set_opt;

#[derive(Debug, Default, PartialEq, Clone, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary, FuzzEq))]
pub struct CommonMIME<'a> {
    pub transfer_encoding: Mechanism<'a>,
    pub id: Option<MessageID<'a>>,
    pub description: Option<Unstructured<'a>>,
}

impl<'a> ContainsUtf8 for CommonMIME<'a> {
    fn contains_utf8(&self) -> bool {
        self.transfer_encoding.contains_utf8()
            || self.id.contains_utf8()
            || self.description.contains_utf8()
    }
}

// Invariant: when T is mime::r#type::Multipart or mime::r#type::Message,
// fields.transfer_encoding must be 7bit, 8bit or binary.
#[derive(Clone, ContainsUtf8, Debug, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub struct MIME<'a, T> {
    pub ctype: T,
    pub fields: CommonMIME<'a>,
}

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for MIME<'a, r#type::Multipart<'a>> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let mime = MIME {
            ctype: u.arbitrary()?,
            fields: u.arbitrary()?,
        };
        match mime.fields.transfer_encoding {
            Mechanism::_7Bit | Mechanism::_8Bit | Mechanism::Binary => (),
            _ => return Err(arbitrary::Error::IncorrectFormat),
        };
        Ok(mime)
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for MIME<'a, r#type::Message<'a>> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let mime = MIME {
            ctype: u.arbitrary()?,
            fields: u.arbitrary()?,
        };
        match mime.fields.transfer_encoding {
            Mechanism::_7Bit | Mechanism::_8Bit | Mechanism::Binary => (),
            _ => return Err(arbitrary::Error::IncorrectFormat),
        };
        Ok(mime)
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for MIME<'a, r#type::Text<'a>> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(MIME {
            ctype: u.arbitrary()?,
            fields: u.arbitrary()?,
        })
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for MIME<'a, r#type::Binary<'a>> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(MIME {
            ctype: u.arbitrary()?,
            fields: u.arbitrary()?,
        })
    }
}

#[derive(Debug, PartialEq, Clone, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary, FuzzEq))]
pub enum AnyMIME<'a> {
    Mult(MIME<'a, r#type::Multipart<'a>>),
    Msg(MIME<'a, r#type::Message<'a>>),
    Txt(MIME<'a, r#type::Text<'a>>),
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

    pub fn get_field(&self, f: field::Entry) -> Option<field::Field<'a>> {
        match f {
            field::Entry::Type => Some(field::Field::Type(self.ctype())),
            field::Entry::TransferEncoding => Some(field::Field::TransferEncoding(
                self.common().transfer_encoding.clone(),
            )),
            field::Entry::ID => self.common().id.clone().map(field::Field::ID),
            field::Entry::Description => self
                .common()
                .description
                .clone()
                .map(field::Field::Description),
        }
    }

    // Returns the list of entries included in this MIME struct. This is used to
    // define the Arbitrary instance for Message and AnyPart, to construct a
    // randomly ordered list of field entries.
    pub fn field_entries(&self) -> HashSet<field::Entry> {
        let mut fs = HashSet::default();
        fs.insert(field::Entry::Type);
        fs.insert(field::Entry::TransferEncoding);
        let common = self.common();
        if common.id.is_some() {
            fs.insert(field::Entry::ID);
        }
        if common.description.is_some() {
            fs.insert(field::Entry::Description);
        }
        fs
    }
}
impl<'a> ContainsUtf8 for AnyMIME<'a> {
    fn contains_utf8(&self) -> bool {
        match self {
            Self::Mult(v) => v.contains_utf8(),
            Self::Msg(v) => v.contains_utf8(),
            Self::Txt(v) => v.contains_utf8(),
            Self::Bin(v) => v.contains_utf8(),
        }
    }
}

impl<'a> From<MIME<'a, r#type::Multipart<'a>>> for AnyMIME<'a> {
    fn from(val: MIME<'a, r#type::Multipart<'a>>) -> Self {
        AnyMIME::Mult(val)
    }
}

impl<'a> From<MIME<'a, r#type::Message<'a>>> for AnyMIME<'a> {
    fn from(val: MIME<'a, r#type::Message<'a>>) -> Self {
        AnyMIME::Msg(val)
    }
}

impl<'a> From<MIME<'a, r#type::Text<'a>>> for AnyMIME<'a> {
    fn from(val: MIME<'a, r#type::Text<'a>>) -> Self {
        AnyMIME::Txt(val)
    }
}
impl<'a> From<MIME<'a, r#type::Binary<'a>>> for AnyMIME<'a> {
    fn from(val: MIME<'a, r#type::Binary<'a>>) -> Self {
        AnyMIME::Bin(val)
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
    pub fn add_field(&mut self, f: NaiveField<'a>) -> Option<field::Entry> {
        match f {
            NaiveField::Type(ctype) => {
                set_opt(&mut self.ctype, ctype).then_some(field::Entry::Type)
            }
            NaiveField::TransferEncoding(enc) => {
                set_opt(&mut self.transfer_encoding, enc).then_some(field::Entry::TransferEncoding)
            }
            NaiveField::ID(id) => set_opt(&mut self.id, id).then_some(field::Entry::ID),
            NaiveField::Description(desc) => {
                set_opt(&mut self.description, desc).then_some(field::Entry::Description)
            }
        }
    }

    pub fn to_interpreted(self, default_type: DefaultType) -> AnyMIME<'a> {
        let typ: AnyType = self
            .ctype
            .as_ref()
            .map(NaiveType::to_type)
            .unwrap_or(default_type.to_type());
        let transfer_encoding = self.transfer_encoding.unwrap_or_default();
        let mut fields = CommonMIME {
            transfer_encoding,
            id: self.id,
            description: self.description,
        };
        match typ {
            AnyType::Multipart(ctype) => {
                // Ensure we are using an encoding allowed for multipart
                fields.transfer_encoding = fields.transfer_encoding.to_multipart_encoding();
                AnyMIME::Mult(MIME { ctype, fields })
            }
            AnyType::Message(ctype) => {
                // Ensure we are using an encoding allowed for message/rfc822
                if let MessageSubtype::RFC822 = ctype.subtype {
                    fields.transfer_encoding =
                        fields.transfer_encoding.to_message_rfc822_encoding();
                }
                // TODO: enforce corresponding restrictions for other message subtypes?
                AnyMIME::Msg(MIME { ctype, fields })
            }
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

#[expect(clippy::wrong_self_convention)]
impl DefaultType {
    fn to_type(self) -> AnyType<'static> {
        match self {
            Self::Generic => AnyType::Text(Default::default()),
            Self::Digest => AnyType::Message(Default::default()),
        }
    }
}
