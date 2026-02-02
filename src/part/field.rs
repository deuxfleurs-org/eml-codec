use bounded_static::ToStatic;

use crate::header;
use crate::mime;

/// Header fields of a generic MIME entity (a MIME entity that is not a toplevel
/// message). Contains either MIME-defined fields or unstructured fields.
#[derive(Debug, Default, PartialEq, ToStatic)]
pub(crate) struct EntityFields<'a> {
    pub mime: mime::NaiveMIME<'a>,
    pub all_fields: Vec<EntityField<'a>>,
}

#[derive(Clone, Debug, PartialEq, ToStatic)]
pub enum EntityField<'a> {
    MIME(mime::field::Entry),
    Unstructured(header::Unstructured<'a>),
}

impl<'a> FromIterator<header::FieldRaw<'a>> for EntityFields<'a> {
    fn from_iter<I: IntoIterator<Item = header::FieldRaw<'a>>>(it: I) -> Self {
        let mut e: EntityFields<'a> = Default::default();
        for f in it {
            if let Ok(mimef) = mime::field::Content::try_from(&f) {
                if let Some(entry) = e.mime.add_field(mimef) {
                    e.all_fields.push(EntityField::MIME(entry))
                } // otherwise drop the field
                continue;
            }

            if let Some(u) = header::Unstructured::from_raw(f) {
                e.all_fields.push(EntityField::Unstructured(u));
            } // otherwise drop the field
        }
        e
    }
}
