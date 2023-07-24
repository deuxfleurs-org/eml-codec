/// Errors triggered when parsing email

#[derive(Debug, PartialEq)]
pub enum EMLError<'a> {
    ParseError(nom::Err<nom::error::Error<&'a [u8]>>),
}
