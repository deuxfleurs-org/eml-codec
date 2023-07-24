use nom::{branch::alt, combinator::map, IResult};

use crate::imf;
use crate::mime;
use crate::part::CompFieldList;

pub enum MixedField<'a> {
    MIME(mime::field::Content<'a>),
    IMF(imf::field::Field<'a>),
}
#[allow(dead_code)]
impl<'a> MixedField<'a> {
    pub fn mime(&self) -> Option<&mime::field::Content<'a>> {
        match self {
            Self::MIME(v) => Some(v),
            _ => None,
        }
    }
    pub fn to_mime(self) -> Option<mime::field::Content<'a>> {
        match self {
            Self::MIME(v) => Some(v),
            _ => None,
        }
    }
    pub fn imf(&self) -> Option<&imf::field::Field<'a>> {
        match self {
            Self::IMF(v) => Some(v),
            _ => None,
        }
    }
    pub fn to_imf(self) -> Option<imf::field::Field<'a>> {
        match self {
            Self::IMF(v) => Some(v),
            _ => None,
        }
    }
}
impl<'a> CompFieldList<'a, MixedField<'a>> {
    pub fn sections(self) -> (mime::AnyMIME<'a>, imf::Imf<'a>) {
        let k = self.known();
        let (v1, v2): (Vec<MixedField>, Vec<MixedField>) =
            k.into_iter().partition(|v| v.mime().is_some());
        let mime = v1
            .into_iter()
            .filter_map(|v| v.to_mime())
            .collect::<mime::AnyMIME>();
        let imf = v2
            .into_iter()
            .filter_map(|v| v.to_imf())
            .collect::<imf::Imf>();
        (mime, imf)
    }
}
pub fn mixed_field(input: &[u8]) -> IResult<&[u8], MixedField> {
    alt((
        map(mime::field::content, MixedField::MIME),
        map(imf::field::field, MixedField::IMF),
    ))(input)
}
