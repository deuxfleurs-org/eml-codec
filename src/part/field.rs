use bounded_static::ToStatic;

use crate::header;
use crate::mime;

/// Header fields of a generic MIME entity (a MIME entity that is not a toplevel
/// message). Contains either MIME-defined fields or unstructured fields.
#[derive(Debug, Default, PartialEq, ToStatic)]
pub(crate) struct EntityFields<'a> {
    pub mime: mime::NaiveMIME<'a>,
    pub entries: Vec<EntityEntry<'a>>,
}

#[derive(Clone, Debug, PartialEq, ToStatic)]
pub enum EntityEntry<'a> {
    MIME(mime::field::Entry),
    Unstructured(header::Unstructured<'a>),
}

impl<'a> FromIterator<header::FieldRaw<'a>> for EntityFields<'a> {
    fn from_iter<I: IntoIterator<Item = header::FieldRaw<'a>>>(it: I) -> Self {
        let mut e: EntityFields<'a> = Default::default();
        for f in it {
            match mime::field::Content::try_from(&f) {
                Ok(mimef) => {
                    if let Some(entry) = e.mime.add_field(mimef) {
                        e.entries.push(EntityEntry::MIME(entry))
                    }; // otherwise drop the field
                    continue;
                },
                Err(mime::field::InvalidField::Body) => {
                    // this is a MIME field but its body is invalid; drop it.
                    continue;
                },
                Err(mime::field::InvalidField::Name) => {
                    // not a MIME field
                    ()
                }
            };

            if let Some(u) = header::Unstructured::from_raw(f) {
                e.entries.push(EntityEntry::Unstructured(u));
            } // otherwise drop the field
        }
        e
    }
}
