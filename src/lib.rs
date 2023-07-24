#![doc = include_str!("../README.md")]

pub mod error;
mod header;
pub mod imf;
pub mod mime;
pub mod part;
pub mod text;

pub fn email(input: &[u8]) -> Result<part::composite::Message, error::EMLError> {
    part::composite::message(mime::Message::default())(input)
        .map(|(_, v)| v)
        .map_err(error::EMLError::ParseError)
}

pub fn imf(input: &[u8]) -> Result<imf::Imf, error::EMLError> {
    imf::field::imf(input)
        .map(|(_, v)| v)
        .map_err(error::EMLError::ParseError)
}
