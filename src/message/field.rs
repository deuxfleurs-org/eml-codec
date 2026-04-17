use bounded_static::ToStatic;
#[cfg(feature = "tracing")]
use tracing::warn;

#[cfg(feature = "arbitrary")]
use crate::fuzz_eq::FuzzEq;
use crate::{header, imf, mime};
use crate::print::{Print, Formatter};

/// Header field of a toplevel message.
/// Is either an Imf field (RFC 5322),
/// MIME-defined fields (RFC 2045),
/// or an unstructured field.
#[derive(Clone, Debug, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub enum MessageField<'a> {
    MIME(mime::field::Field<'a>),
    Imf(imf::field::Field<'a>),
    // invariant: has a field name that is different from IMF or MIME headers.
    Unstructured(header::Unstructured<'a>),
}

impl<'a> Print for MessageField<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        match self {
            MessageField::MIME(f) => f.print(fmt),
            MessageField::Imf(f) => f.print(fmt),
            MessageField::Unstructured(u) => u.print(fmt),
        }
    }
}

/// Entry for a header field of a toplevel message.
#[derive(Clone, Debug, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub enum MessageEntry<'a> {
    MIME(mime::field::Entry),
    Imf(imf::field::Entry),
    // invariant: has a field name that is different from IMF or MIME headers.
    Unstructured(header::Unstructured<'a>),
}

/// Collects fields and entries for a toplevel message. Only for eml-codec's
/// internal use.
#[derive(Debug, PartialEq, ToStatic)]
pub(crate) struct NaiveMessageFields<'a> {
    pub mime: mime::NaiveMIME<'a>,
    pub imf: imf::Imf<'a>,
    pub entries: Vec<MessageEntry<'a>>,
}

impl<'a> FromIterator<header::FieldRaw<'a>> for NaiveMessageFields<'a> {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "MessageFields::from_iter", skip(it))
    )]
    fn from_iter<I: IntoIterator<Item = header::FieldRaw<'a>>>(it: I) -> Self {
        let mut mime = mime::NaiveMIME::default();
        let mut imf = imf::PartialImf::default();
        let mut entries = vec![];
        for f in it {
            match mime::field::NaiveField::try_from(&f) {
                Ok(mimef) => {
                    if let Some(entry) = mime.add_field(mimef) {
                        entries.push(MessageEntry::MIME(entry))
                    } else {
                        // otherwise drop the field
                        #[cfg(feature = "tracing-recover")]
                        warn!(field = ?f, "dropping conflicting MIME field")
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

            match imf::field::Field::try_from(&f) {
                Ok(imff) => {
                    match imf.add_field(imff) {
                        Ok(entry) =>
                            entries.push(MessageEntry::Imf(entry)),
                        Err(imf::AddFieldErr::NoEntry) => {
                            #[cfg(feature = "tracing-recover")]
                            warn!(field = ?f, "no new entry for IMF field");
                        },
                        Err(imf::AddFieldErr::Conflict) => {
                            #[cfg(feature = "tracing-recover")]
                            warn!(field = ?f, "discarding conflicting IMF field");
                        },
                    }
                    continue;
                },
                Err(imf::field::InvalidField::NeedsDiscard) => {
                    // this is an IMF field for which we recognized the body, but the
                    // body isn't RFC compliant and the fields needs to be dropped.
                    #[cfg(feature = "tracing-recover")]
                    warn!(field = ?f, "dropping IMF field with a body to be discarded");
                    continue;
                }
                Err(imf::field::InvalidField::Body) => {
                    // this is an IMF field but its body is invalid; drop it.
                    #[cfg(feature = "tracing-unsupported")]
                    warn!(field = ?f, "dropping IMF field with an invalid body");
                    continue;
                }
                Err(imf::field::InvalidField::Name) => {
                    // not an IMF field
                    ()
                }
            }

            if let Some(u) = header::Unstructured::from_raw(&f) {
                entries.push(MessageEntry::Unstructured(u));
            } else {
                // otherwise drop the field
                #[cfg(feature = "tracing-unsupported")]
                warn!(field = ?f, "dropping field that cannot be parsed as unstructured")
            }
        }
        entries.extend(
            imf.missing_mandatory_fields()
               .into_iter()
               .map(MessageEntry::Imf)
        );

        NaiveMessageFields {
            mime,
            imf: imf.to_imf(),
            entries,
        }
    }
}
