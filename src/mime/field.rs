#[derive(Debug, PartialEq)]
pub enum Content<'a> {
    Type(Type<'a>),
    TransferEncoding(Mechanism<'a>),
    ID(MessageId<'a>),
    Description(Unstructured),
}

fn field(input: &str) -> IResult<&str, Content> {
    terminated(alt((
        preceded(field_name(b"content-type"), map(date, Field::Date)),

    field_name(input).map(|(rest, name)| {
        (
            "",
            match name.to_lowercase().as_ref() {
                "" => ContentType(Type(rest)),
                "content-transfer-encoding" => ContentTransferEncoding(Mechanism(rest)),
                "content-id" => ContentID(Identifier(rest)),
                "content-description" => ContentDescription(Unstructured(rest)),
                _ => Optional(name, Unstructured(rest)),
            }
        )
    })
}
