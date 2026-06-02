use bounded_static::ToStatic;
#[cfg(feature = "tracing")]
use tracing::warn;

#[cfg(feature = "arbitrary")]
use crate::fuzz_eq::FuzzEq;
use crate::print::{Formatter, Print};
use crate::raw_input::RawInput;
use crate::{header, imf, mime};

/// Header field of a toplevel message.
/// Is either an Imf field (RFC 5322),
/// MIME-defined fields (RFC 2045),
/// or an unstructured field.
#[derive(Clone, Debug, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub enum MessageField<'a> {
    MIME {
        f: mime::field::Field<'a>,
        raw_body: RawInput<'a>,
    },
    Imf {
        f: imf::field::Field<'a>,
        raw_body: RawInput<'a>,
    },
    // invariant: has a field name that is different from IMF or MIME headers.
    Unstructured(header::Unstructured<'a>),
}

impl<'a> MessageField<'a> {
    pub fn raw_name(&self) -> header::FieldName<'a> {
        match self {
            MessageField::MIME { f, .. } => f.raw_name(),
            MessageField::Imf { f, .. } => f.raw_name(),
            MessageField::Unstructured(u) => u.name.clone(),
        }
    }

    pub fn raw_body(&self) -> RawInput<'a> {
        match self {
            MessageField::MIME { raw_body, .. } => raw_body.clone(),
            MessageField::Imf { raw_body, .. } => raw_body.clone(),
            MessageField::Unstructured(u) => u.raw_body.clone(),
        }
    }
}

impl<'a> Print for MessageField<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        match self {
            MessageField::MIME { f, .. } => f.print(fmt),
            MessageField::Imf { f, .. } => f.print(fmt),
            MessageField::Unstructured(u) => u.print(fmt),
        }
    }
}

/// Entry for a header field of a toplevel message.
#[derive(Clone, Debug, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub enum MessageEntry<'a> {
    MIME {
        e: mime::field::Entry,
        raw_body: RawInput<'a>,
    },
    Imf {
        e: imf::field::Entry,
        raw_body: RawInput<'a>,
    },
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
                        entries.push(MessageEntry::MIME {
                            e: entry,
                            raw_body: f.body.into(),
                        })
                    } else {
                        // otherwise drop the field
                        #[cfg(feature = "tracing-recover")]
                        warn!(field = ?f, "dropping conflicting MIME field")
                    }
                    continue;
                }
                Err(mime::field::InvalidField::Body) => {
                    // this is a MIME field but its body is invalid; drop it.
                    #[cfg(feature = "tracing-unsupported")]
                    warn!(field = ?f, "dropping MIME field with an invalid body");
                    continue;
                }
                Err(mime::field::InvalidField::Name) => {
                    // not a MIME field
                    ()
                }
            };

            match imf::field::Field::try_from(&f) {
                Ok(imff) => {
                    match imf.add_field(imff) {
                        Ok(entry) => entries.push(MessageEntry::Imf {
                            e: entry,
                            raw_body: f.body.into(),
                        }),
                        Err(imf::AddFieldErr::NoEntry) => {
                            #[cfg(feature = "tracing-recover")]
                            warn!(field = ?f, "no new entry for IMF field");
                        }
                        Err(imf::AddFieldErr::Conflict) => {
                            #[cfg(feature = "tracing-recover")]
                            warn!(field = ?f, "discarding conflicting IMF field");
                        }
                    }
                    continue;
                }
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

        NaiveMessageFields {
            mime,
            imf: imf.to_imf(),
            entries,
        }
    }
}
