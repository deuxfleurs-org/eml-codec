use crate::fragments::lazy;
use crate::multipass::extract_fields;
use crate::multipass::field_eager;

#[derive(Debug, PartialEq)]
pub struct Parsed<'a> {
    pub fields: Vec<lazy::Field<'a>>,
    pub body: &'a [u8],
}

pub fn new<'a>(ef: &'a extract_fields::Parsed<'a>) -> Parsed<'a> {
    Parsed {
        fields: ef.fields.iter().map(|e| (*e).into()).collect(),
        body: ef.body,
    }
}

impl<'a> Parsed<'a> {
    pub fn body(&'a self) -> field_eager::Parsed<'a> {
        field_eager::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_name() {
        assert_eq!(
            new(&extract_fields::Parsed {
                fields: vec![
                    "From: hello@world.com,\r\n\talice@wonderlands.com\r\n",
                    "Date: 12 Mar 1997 07:33:25 Z\r\n",
                ],
                body: b"Hello world!",
            }),
            Parsed {
                fields: vec![
                    lazy::Field::From(lazy::MailboxList(
                        "hello@world.com,\r\n\talice@wonderlands.com\r\n"
                    )),
                    lazy::Field::Date(lazy::DateTime("12 Mar 1997 07:33:25 Z\r\n")),
                ],
                body: b"Hello world!",
            }
        );
    }

    #[test]
    fn test_mime_fields() {
        assert_eq!(
            new(&extract_fields::Parsed {
                fields: vec![
                    "MIME-Version: 1.0 \r\n",
                    "Content-Type: multipart/alternative; boundary=\"bound\"\r\n",
                    "Content-Transfer-Encoding: 7bit\r\n",
                    "Content-ID: <foo4*foo1@bar.net>\r\n",
                    "Content-Description: hello world\r\n",
                ],
                body: b"Hello world!",
            }),
            Parsed {
                fields: vec![
                    lazy::Field::MIMEVersion(lazy::Version("1.0 \r\n")),
                    lazy::Field::MIME(lazy::MIMEField::ContentType(lazy::Type("multipart/alternative; boundary=\"bound\"\r\n"))),
                    lazy::Field::MIME(lazy::MIMEField::ContentTransferEncoding(lazy::Mechanism("7bit\r\n"))),
                    lazy::Field::MIME(lazy::MIMEField::ContentID(lazy::Identifier("<foo4*foo1@bar.net>\r\n"))),
                    lazy::Field::MIME(lazy::MIMEField::ContentDescription(lazy::Unstructured("hello world\r\n"))),
                ],
                body: b"Hello world!",
            }
        );
    }
}