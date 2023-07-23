mod error;
mod header;
mod mime;
mod part;
mod rfc5322;
mod text;

pub fn email(input: &[u8]) -> Result<part::part::Message, error::EMLError> {
    part::part::message(mime::mime::Message::default())(input)
        .map(|(_, v)| v)
        .map_err(error::EMLError::ParseError)
}

pub fn imf(input: &[u8]) -> Result<rfc5322::message::Message, error::EMLError> {
    rfc5322::field::message(input)
        .map(|(_, v)| v)
        .map_err(error::EMLError::ParseError)
}
