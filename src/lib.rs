#![doc = include_str!("../README.md")]

mod error;
mod header;
mod imf;
mod mime;
mod part;
mod text;

pub fn email(input: &[u8]) -> Result<part::composite::Message, error::EMLError> {
    part::composite::message(mime::mime::Message::default())(input)
        .map(|(_, v)| v)
        .map_err(error::EMLError::ParseError)
}

pub fn imf(input: &[u8]) -> Result<imf::Imf, error::EMLError> {
    imf::field::imf(input)
        .map(|(_, v)| v)
        .map_err(error::EMLError::ParseError)
}
