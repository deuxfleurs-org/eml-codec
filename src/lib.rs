pub mod error;
pub mod text;
pub mod header;
pub mod rfc5322;
pub mod mime;
pub mod part;

/*
use crate::part;
use crate::mime;
use crate::rfc5322 as imf;
use crate::header;

pub fn email(input: &[u8]) -> Result<part::part::Message> {
    message(mime::mime::Message::default())(input).map(|(_, v)| v)
}

pub fn imf(input: &[u8]) -> Result<imf::message::Message> {
    header::header(imf::field::field)
    map(header(field), |v| FieldList(v.known()).message())(fullmail)
}*/
