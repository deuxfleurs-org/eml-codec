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
use crate::i18n::ContainsUtf8;
use crate::imf::identification::MessageID;
use crate::mime::field::{Content, Entry as FieldEntry};
use crate::mime::mechanism::Mechanism;
use crate::mime::r#type::{AnyType, NaiveType};
use crate::print::Formatter;
use crate::text::misc_token::Unstructured;
use crate::utils::set_opt;

#[derive(Debug, Default, PartialEq, Clone, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub struct CommonMIME<'a> {
    pub transfer_encoding: Mechanism<'a>,
    pub id: Option<MessageID<'a>>,
    pub description: Option<Unstructured<'a>>,

    // This field is for information only, and should not be considered part of
    // the "structured" MIME AST. It contains fields encountered during parsing
    // that were discarded because they conflicted with an earlier field (e.g.
    // because there were multiple occurrences of a field that must only appear
    // once). These discarded fields are never printed back.
    #[cfg_attr(feature = "arbitrary", fuzz_eq(ignore))]
    pub discarded: Vec<Content<'a>>,
}

impl<'a> ContainsUtf8 for CommonMIME<'a> {
    fn contains_utf8(&self) -> bool {
        self.transfer_encoding.contains_utf8() ||
        self.id.contains_utf8() ||
        self.description.contains_utf8()
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for CommonMIME<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self {
            transfer_encoding: u.arbitrary()?,
            id: u.arbitrary()?,
            description: u.arbitrary()?,
            discarded: vec![],
        })
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
                    header::print_unstructured(fmt, b"Content-Description", desc)
                }
            },
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

impl<'a> Into<AnyMIME<'a>> for MIME<'a, r#type::Text<'a>> {
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
    #[cfg_attr(feature = "arbitrary", fuzz_eq(ignore))]
    discarded: Vec<Content<'a>>,
}

impl<'a> NaiveMIME<'a> {
    pub fn add_field(&mut self, f: Content<'a>) -> Option<FieldEntry> {
        // XXX it is slightly unfortunate that we must .clone() just for the
        // `is_none()` case below
        let res = self.add_field_inner(f.clone());

        // Store dropped fields in `self.discarded` for information purposes
        if res.is_none() {
            self.discarded.push(f);
        }

        res
    }

    fn add_field_inner(&mut self, f: Content<'a>) -> Option<FieldEntry> {
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
            .unwrap_or_default();
        let mut fields = CommonMIME {
            transfer_encoding,
            id: self.id,
            description: self.description,
            discarded: self.discarded,
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
