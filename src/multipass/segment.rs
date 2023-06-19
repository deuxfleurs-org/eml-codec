#[derive(Debug, PartialEq)]
pub struct Segment<'a> {
    pub header: &'a [u8],
    pub body: &'a [u8],
}


