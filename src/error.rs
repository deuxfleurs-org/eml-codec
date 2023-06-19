use nom;

#[derive(Debug, PartialEq)]
pub enum IMFError<'a> {
    Segment(nom::Err<nom::error::Error<&'a [u8]>>),
    ExtractFields(nom::Err<nom::error::Error<&'a str>>),
}
