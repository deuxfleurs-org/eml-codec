use crate::mime::r#type::NaiveType;
use crate::mime::mechanism::Mechanism;
use crate::rfc5322::identification::MessageID;
use crate::text::misc_token::Unstructured;
use crate::mime::field::Content;
use crate::mime::charset::EmailCharset;

#[derive(Debug, PartialEq, Default)]
pub struct MIME<'a> {
    pub part_type: Type,
    pub transfer_encoding: Mechanism<'a>,
    pub id: Option<MessageID<'a>>,
    pub description: Option<Unstructured<'a>>,
}

impl<'a> FromIterator<Content<'a>> for MIME<'a> {
    fn from_iter<I: IntoIterator<Item = Content<'a>>>(source: I) -> Self {
        source.into_iter().fold(
            MIME::default(),
            |mut section, field| {
                match field {
                    Content::Type(v) => section.part_type = v.to_type(),
                    Content::TransferEncoding(v) => section.transfer_encoding = v,
                    Content::ID(v) => section.id = Some(v),
                    Content::Description(v) => section.description = Some(v),
                };
                section
            }
        )
    }
}

// -------- TYPE
#[derive(Debug, PartialEq)]
pub enum Type {
    // Composite types
    Multipart(Multipart),
    Message(Message),

    // Discrete types
    Text(Text),
    Binary,
}
impl Default for Type {
    fn default() -> Self {
        Self::Text(Text::default())
    }
}
impl<'a> From<&'a NaiveType<'a>> for Type {
    fn from(nt: &'a NaiveType<'a>) -> Self {
        match nt.main.to_ascii_lowercase().as_slice() {
            b"multipart" => Multipart::try_from(nt).map(Self::Multipart).unwrap_or(Self::default()),
            b"message" => Self::Message(Message::from(nt)),
            b"text" => Self::Text(Text::from(nt)),
            _ => Self::Binary,
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Multipart {
    pub subtype: MultipartSubtype,
    pub boundary: String,
}
impl<'a> TryFrom<&'a NaiveType<'a>> for Multipart {
    type Error = ();

    fn try_from(nt: &'a NaiveType<'a>) -> Result<Self, Self::Error> {
        nt.params.iter()
            .find(|x| x.name.to_ascii_lowercase().as_slice() == b"boundary")
            .map(|boundary| Multipart {
                subtype: MultipartSubtype::from(nt),
                boundary: boundary.value.to_string(),
            })
            .ok_or(())
    }
}

#[derive(Debug, PartialEq)]
pub enum MultipartSubtype {
    Alternative,
    Mixed,
    Digest,
    Parallel,
    Report,
    Unknown,
}
impl<'a> From<&NaiveType<'a>> for MultipartSubtype {
    fn from(nt: &NaiveType<'a>) -> Self {
        match nt.sub.to_ascii_lowercase().as_slice() {
            b"alternative" => Self::Alternative,
            b"mixed" => Self::Mixed,
            b"digest" => Self::Digest,
            b"parallel" => Self::Parallel,
            b"report" => Self::Report,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Message {
    RFC822,
    Partial,
    External,
    Unknown,
}
impl<'a> From<&NaiveType<'a>> for Message {
    fn from(nt: &NaiveType<'a>) -> Self {
        match nt.sub.to_ascii_lowercase().as_slice() {
            b"rfc822" => Self::RFC822,
            b"partial" => Self::Partial,
            b"external" => Self::External,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, PartialEq, Default)]
pub struct Text {
    pub subtype: TextSubtype,
    pub charset: EmailCharset,
}
impl<'a> From<&NaiveType<'a>> for Text {
    fn from(nt: &NaiveType<'a>) -> Self {
        Self {
            subtype: TextSubtype::from(nt),
            charset: nt.params.iter()
                .find(|x| x.name.to_ascii_lowercase().as_slice() == b"charset")
                .map(|x| EmailCharset::from(x.value.to_string().as_bytes()))
                .unwrap_or(EmailCharset::US_ASCII),
        }
    }
}

#[derive(Debug, PartialEq, Default)]
pub enum TextSubtype {
    #[default]
    Plain,
    Html,
    Unknown,
}
impl<'a> From<&NaiveType<'a>> for TextSubtype {
    fn from(nt: &NaiveType<'a>) -> Self {
        match nt.sub.to_ascii_lowercase().as_slice() {
            b"plain" => Self::Plain,
            b"html" => Self::Html,
            _ => Self::Unknown,
        }
    }
}
