use bounded_static::ToStatic;
#[cfg(feature = "tracing")]
use tracing::warn;

#[cfg(feature = "arbitrary")]
use crate::fuzz_eq::FuzzEq;
use crate::header;
use crate::mime;
use crate::print::{Print, Formatter};
use crate::raw_input::RawInput;

/// Header field of a generic MIME entity (a MIME entity that is not a toplevel
/// message). Is either a MIME-defined field or an unstructured field.
#[derive(Clone, Debug, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub enum EntityField<'a> {
    MIME { f: mime::field::Field<'a>, raw_body: RawInput<'a> },
    Unstructured(header::Unstructured<'a>),
}

impl<'a> EntityField<'a> {
    pub fn raw_name(&self) -> header::FieldName<'a> {
        match self {
            EntityField::MIME { f, .. } => f.raw_name(),
            EntityField::Unstructured(u) => u.name.clone(),
        }
    }

    pub fn raw_body(&self) -> RawInput<'a> {
        match self {
            EntityField::MIME { raw_body, .. } => raw_body.clone(),
            EntityField::Unstructured(u) => u.raw_body.clone(),
        }
    }
}

impl<'a> Print for EntityField<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        match self {
            EntityField::MIME { f, .. } => f.print(fmt),
            EntityField::Unstructured(u) => u.print(fmt),
        }
    }
}

/// Entry for an entity field.
#[derive(Clone, Debug, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub enum EntityEntry<'a> {
    MIME { e: mime::field::Entry, raw_body: RawInput<'a> },
    Unstructured(header::Unstructured<'a>),
}

/// Collects fields and entries for a generic MIME entity. Only for eml-codec's
/// internal use.
#[derive(Debug, Default, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub(crate) struct NaiveEntityFields<'a> {
    pub mime: mime::NaiveMIME<'a>,
    pub entries: Vec<EntityEntry<'a>>,
}

impl<'a> FromIterator<header::FieldRaw<'a>> for NaiveEntityFields<'a> {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "EntityFields::from_iter", skip(it))
    )]
    fn from_iter<I: IntoIterator<Item = header::FieldRaw<'a>>>(it: I) -> Self {
        let mut e: NaiveEntityFields<'a> = Default::default();
        for f in it {
            match mime::field::NaiveField::try_from(&f) {
                Ok(mimef) => {
                    if let Some(entry) = e.mime.add_field(mimef) {
                        e.entries.push(EntityEntry::MIME { e: entry, raw_body: f.body.into() })
                    } else {
                        // otherwise drop the field
                        #[cfg(feature = "tracing-recover")]
                        warn!(field = ?f, "dropping conflicting MIME field");
                    }
                    continue;
                },
                Err(mime::field::InvalidField::Body) => {
                    // this is a MIME field but its body is invalid; drop it.
                    #[cfg(feature = "tracing-unsupported")]
                    warn!(field = ?f, "dropping MIME field with an invalid body");
                    continue;
                },
                Err(mime::field::InvalidField::Name) => {
                    // not a MIME field
                    ()
                }
            };

            if let Some(u) = header::Unstructured::from_raw(&f) {
                e.entries.push(EntityEntry::Unstructured(u));
            } else {
                // otherwise drop the field
                #[cfg(feature = "tracing-unsupported")]
                warn!(field = ?f, "dropping field which cannot be parsed as unstructured");
            }
        }
        e
    }
}
