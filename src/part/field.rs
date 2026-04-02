use bounded_static::ToStatic;
#[cfg(feature = "tracing")]
use tracing::warn;

#[cfg(feature = "arbitrary")]
use crate::fuzz_eq::FuzzEq;
use crate::header;
use crate::mime;

/// Header fields of a generic MIME entity (a MIME entity that is not a toplevel
/// message). Contains either MIME-defined fields or unstructured fields.
#[derive(Debug, Default, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub(crate) struct EntityFields<'a> {
    pub mime: mime::NaiveMIME<'a>,
    pub entries: Vec<EntityEntry<'a>>,
}

#[derive(Clone, Debug, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub enum EntityEntry<'a> {
    MIME(mime::field::Entry),
    Unstructured(header::Unstructured<'a>),
}

impl<'a> FromIterator<header::FieldRaw<'a>> for EntityFields<'a> {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", name = "EntityFields::from_iter", skip(it))
    )]
    fn from_iter<I: IntoIterator<Item = header::FieldRaw<'a>>>(it: I) -> Self {
        let mut e: EntityFields<'a> = Default::default();
        for f in it {
            match mime::field::Content::try_from(&f) {
                Ok(mimef) => {
                    if let Some(entry) = e.mime.add_field(mimef) {
                        e.entries.push(EntityEntry::MIME(entry))
                    } else {
                        // otherwise drop the field
                        #[cfg(feature = "tracing-recover")]
                        warn!(field = ?f, "dropping conflicting MIME field");
                    }
                    continue;
                },
                Err(mime::field::InvalidField::Body) => {
                    // this is a MIME field but its body is invalid; drop it.
                    #[cfg(feature = "tracing-discard")]
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
                #[cfg(feature = "tracing-discard")]
                warn!(field = ?f, "dropping field which cannot be parsed as unstructured");
            }
        }
        e
    }
}
