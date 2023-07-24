mod error;
mod header;
mod mime;
mod part;
mod imf;
mod text;

pub fn email(input: &[u8]) -> Result<part::part::Message, error::EMLError> {
    part::part::message(mime::mime::Message::default())(input)
        .map(|(_, v)| v)
        .map_err(error::EMLError::ParseError)
}

pub fn imf(input: &[u8]) -> Result<imf::Imf, error::EMLError> {
    imf::field::imf(input)
        .map(|(_, v)| v)
        .map_err(error::EMLError::ParseError)
}
