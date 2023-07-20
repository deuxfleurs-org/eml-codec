use nom::{
    IResult,
    branch::alt,
    bytes::complete::tag_no_case,
    combinator::{map, opt, value},
    sequence::delimited,
};
use crate::text::whitespace::cfws;
use crate::text::words::mime_token as token;

#[derive(Debug, Clone, PartialEq)]
pub enum DecodedMechanism<'a> {
    _7Bit,
    _8Bit,
    Binary,
    QuotedPrintable,
    Base64,
    Other(&'a [u8]),
}

pub struct Mechanism<'a>(pub &'a [u8]);
impl<'a> Mechanism<'a> {
    pub fn decode(&self) -> DecodedMechanism {
        use DecodedMechanism::*;
        match self.0.to_ascii_lowercase().as_slice() {
            b"7bit" => _7Bit,
            b"8bit" => _8Bit,
            b"binary" => Binary,
            b"quoted-printable" => QuotedPrintable,
            b"base64" => Base64,
            _ => Other(self.0),
        }
    }
}

pub fn mechanism(input: &[u8]) -> IResult<&[u8], Mechanism> {
    map(token, Mechanism)(input)
}


#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_mechanism() {
        assert_eq!(
            mechanism(b"7bit").unwrap().1.decode(),
            DecodedMechanism::_7Bit,
        );

        assert_eq!(
            mechanism(b"(youhou) 8bit").unwrap().1.decode(),
            DecodedMechanism::_8Bit,
        );
        
        assert_eq!(
            mechanism(b"(blip) bInArY (blip blip)").unwrap().1.decode(),
            DecodedMechanism::Binary,
        );

        assert_eq!(
            mechanism(b" base64 ").unwrap().1.decode(),
            DecodedMechanism::Base64,
        );

        assert_eq!(
            mechanism(b" Quoted-Printable ").unwrap().1.decode(),
            DecodedMechanism::QuotedPrintable,
        );
    }
}
