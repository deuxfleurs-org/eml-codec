use crate::mime::r#type::NaiveType;
use crate::mime::mechanism::Mechanism;
use crate::rfc5322::identification::MessageID;
use crate::text::misc_token::Unstructured;
use crate::mime::field::Content;

#[derive(Debug, PartialEq, Default)]
pub struct MIME<'a> {
    pub content_type: Option<NaiveType<'a>>,
    pub content_transfer_encoding: Option<Mechanism<'a>>,
    pub content_id: Option<MessageID<'a>>,
    pub content_description: Option<Unstructured<'a>>,
}

impl<'a> FromIterator<Content<'a>> for MIME<'a> {
    fn from_iter<I: IntoIterator<Item = Content<'a>>>(source: I) -> Self {
        source.into_iter().fold(
            MIME::default(),
            |mut section, field| {
                match field {
                    Content::Type(v) => section.content_type = Some(v),
                    Content::TransferEncoding(v) => section.content_transfer_encoding = Some(v),
                    Content::ID(v) => section.content_id = Some(v),
                    Content::Description(v) => section.content_description = Some(v),
                };
                section
            }
        )
    }
}
