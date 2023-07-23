use nom;

#[derive(Debug, PartialEq)]
pub enum EMLError<'a> {
    ParseError(nom::Err<nom::error::Error<&'a [u8]>>),
}
