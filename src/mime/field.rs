use nom::{
    branch::alt,
    combinator::map,
    sequence::{preceded, terminated},
    IResult,
};

use crate::header::{field_name};
use crate::imf::identification::{msg_id, MessageID};
use crate::mime::mechanism::{mechanism, Mechanism};
use crate::mime::r#type::{naive_type, NaiveType};
use crate::text::misc_token::{unstructured, Unstructured};
use crate::text::whitespace::obs_crlf;

#[derive(Debug, PartialEq)]
pub enum Content<'a> {
    Type(NaiveType<'a>),
    TransferEncoding(Mechanism<'a>),
    ID(MessageID<'a>),
    Description(Unstructured<'a>),
}
#[allow(dead_code)]
impl<'a> Content<'a> {
    pub fn ctype(&'a self) -> Option<&'a NaiveType<'a>> {
        match self {
            Content::Type(v) => Some(v),
            _ => None,
        }
    }
    pub fn transfer_encoding(&'a self) -> Option<&'a Mechanism<'a>> {
        match self {
            Content::TransferEncoding(v) => Some(v),
            _ => None,
        }
    }
    pub fn id(&'a self) -> Option<&'a MessageID<'a>> {
        match self {
            Content::ID(v) => Some(v),
            _ => None,
        }
    }
    pub fn description(&'a self) -> Option<&'a Unstructured<'a>> {
        match self {
            Content::Description(v) => Some(v),
            _ => None,
        }
    }
}

/*
pub fn to_mime<'a, T: WithDefaultType>(list: Vec<Content<'a>>) -> AnyMIMEWithDefault<'a, T> {
    list.into_iter().collect::<AnyMIMEWithDefault<T>>()
}*/

pub fn content(input: &[u8]) -> IResult<&[u8], Content> {
    terminated(
        alt((
            preceded(field_name(b"content-type"), map(naive_type, Content::Type)),
            preceded(
                field_name(b"content-transfer-encoding"),
                map(mechanism, Content::TransferEncoding),
            ),
            preceded(field_name(b"content-id"), map(msg_id, Content::ID)),
            preceded(
                field_name(b"content-description"),
                map(unstructured, Content::Description),
            ),
        )),
        obs_crlf,
    )(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::{header};
    use crate::mime::charset::EmailCharset;
    use crate::mime::r#type::*;
    use crate::text::misc_token::MIMEWord;
    use crate::text::quoted::QuotedString;

    #[test]
    fn test_content_type() {
        let (rest, content) =
            content(b"Content-Type: text/plain; charset=UTF-8; format=flowed\r\n").unwrap();
        assert_eq!(&b""[..], rest);

        if let Content::Type(nt) = content {
            assert_eq!(
                nt.to_type(),
                AnyType::Text(Deductible::Explicit(Text {
                    charset: Deductible::Explicit(EmailCharset::UTF_8),
                    subtype: TextSubtype::Plain,
                })),
            );
        } else {
            panic!("Expected Content::Type, got {:?}", content);
        }
    }

    #[test]
    fn test_header() {
        let fullmail: &[u8] = r#"Date: Sat, 8 Jul 2023 07:14:29 +0200
From: Grrrnd Zero <grrrndzero@example.org>
To: John Doe <jdoe@machine.example>
Subject: Re: Saying Hello
Message-ID: <NTAxNzA2AC47634Y366BAMTY4ODc5MzQyODY0ODY5@www.grrrndzero.org>
MIME-Version: 1.0
Content-Type: multipart/alternative;
 boundary="b1_e376dc71bafc953c0b0fdeb9983a9956"
Content-Transfer-Encoding: 7bit

This is a multipart message.

"#
        .as_bytes();

        assert_eq!(
            map(header(content), |(k, _, _)| k)(fullmail),
            Ok((
                &b"This is a multipart message.\n\n"[..],
                vec![
                    Content::Type(NaiveType {
                        main: &b"multipart"[..],
                        sub: &b"alternative"[..],
                        params: vec![Parameter {
                            name: &b"boundary"[..],
                            value: MIMEWord::Quoted(QuotedString(vec![
                                &b"b1_e376dc71bafc953c0b0fdeb9983a9956"[..]
                            ])),
                        }]
                    }),
                    Content::TransferEncoding(Mechanism::_7Bit),
                ],
            )),
        );
    }
}
