use nom::combinator::map;

use crate::header;
use crate::imf::identification::{msg_id, MessageID};
use crate::mime::mechanism::{mechanism, Mechanism};
use crate::mime::r#type::{naive_type, NaiveType};
use crate::text::misc_token::{unstructured, Unstructured};

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

impl<'a> TryFrom<&header::FieldRaw<'a>> for Content<'a> {
    type Error = ();
    fn try_from(f: &header::FieldRaw<'a>) -> Result<Self, Self::Error> {
        let content = match f {
            header::FieldRaw::Good(key, value) => match key.bytes().to_ascii_lowercase().as_slice()
            {
                b"content-type" => map(naive_type, Content::Type)(value),
                b"content-transfer-encoding" => map(mechanism, Content::TransferEncoding)(value),
                b"content-id" => map(msg_id, Content::ID)(value),
                b"content-description" => map(unstructured, Content::Description)(value),
                _ => return Err(()),
            },
            _ => return Err(()),
        };

        //@TODO check that the full value is parsed, otherwise maybe log an error ?!
        content.map(|(_, content)| content).or(Err(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header;
    //use crate::mime::charset::EmailCharset;
    use crate::mime::r#type::*;
    use crate::text::misc_token::MIMEWord;
    use crate::text::quoted::QuotedString;

    /*
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
    }*/

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
            map(header::header_kv, |k| k
                .iter()
                .flat_map(Content::try_from)
                .collect())(fullmail),
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
