use nom::{
    IResult,
    branch::alt,
    combinator::map,
    sequence::{preceded, terminated},
};

use crate::text::whitespace::obs_crlf;
use crate::text::misc_token::{Unstructured, unstructured};
use crate::rfc5322::identification::{MessageID, msg_id};
use crate::header::field_name;
use crate::mime::r#type::{NaiveType, naive_type};
use crate::mime::mechanism::{Mechanism, mechanism};

#[derive(Debug, PartialEq)]
pub enum Content<'a> {
    Type(NaiveType<'a>),
    TransferEncoding(Mechanism<'a>),
    ID(MessageID<'a>),
    Description(Unstructured<'a>),
}

fn field(input: &[u8]) -> IResult<&[u8], Content> {
    terminated(alt((
        preceded(field_name(b"content-type"), map(naive_type, Content::Type)),
        preceded(field_name(b"content-transfer-encoding"), map(mechanism, Content::TransferEncoding)),
        preceded(field_name(b"content-id"), map(msg_id, Content::ID)),
        preceded(field_name(b"content-description"), map(unstructured, Content::Description)),
    )), obs_crlf)(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mime::r#type::*;
    use crate::mime::charset::EmailCharset;

    #[test]
    fn test_content_type() {
        let (rest, content) = field(b"Content-Type: text/plain; charset=UTF-8; format=flowed\r\n").unwrap();
        assert_eq!(&b""[..], rest);

        if let Content::Type(nt) = content {
            assert_eq!(
                nt.to_type(),
                Type::Text(TextDesc {
                    charset: EmailCharset::UTF_8,
                    subtype: TextSubtype::Plain,
                }),
            );
        } else {
            panic!("Expected Content::Type, got {:?}", content);
        }
    }
}
