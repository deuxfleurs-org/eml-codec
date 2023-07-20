#[derive(Debug, PartialEq)]
pub enum Field<'a> {
    ContentType(Type<'a>),
    ContentTransferEncoding(Mechanism<'a>),
    ContentID(MessageId<'a>),
    ContentDescription(Unstructured),
}

fn correct_mime_field(input: &str) -> IResult<&str, MIMEField> {
    use MIMEField::*;
    field_name(input).map(|(rest, name)| {
        (
            "",
            match name.to_lowercase().as_ref() {
                "content-type" => ContentType(Type(rest)),
                "content-transfer-encoding" => ContentTransferEncoding(Mechanism(rest)),
                "content-id" => ContentID(Identifier(rest)),
                "content-description" => ContentDescription(Unstructured(rest)),
                _ => Optional(name, Unstructured(rest)),
            }
        )
    })
}
