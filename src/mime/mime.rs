
#[derive(Debug, PartialEq, Default)]
pub struct MIMESection<'a> {
    pub content_type: Option<&'a Type<'a>>,
    pub content_transfer_encoding: Option<&'a Mechanism<'a>>,
    pub content_id: Option<&'a MessageId<'a>>,
    pub content_description: Option<&'a Unstructured>,
    pub optional: HashMap<&'a str, &'a Unstructured>,
    pub unparsed: Vec<&'a str>,
}


impl<'a> FromIterator<&'a MIMEField<'a>> for MIMESection<'a> {
    fn from_iter<I: IntoIterator<Item = &'a MIMEField<'a>>>(iter: I) -> Self {
        let mut section = MIMESection::default();
        for field in iter {
            match field {
                MIMEField::ContentType(v) => section.content_type = Some(v),
                MIMEField::ContentTransferEncoding(v) => section.content_transfer_encoding = Some(v),
                MIMEField::ContentID(v) => section.content_id = Some(v),
                MIMEField::ContentDescription(v) => section.content_description = Some(v),
                MIMEField::Optional(k, v) => { section.optional.insert(k, v); },
                MIMEField::Rescue(v) => section.unparsed.push(v),
            };
        }
        section
    }
}
