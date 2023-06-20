use crate::fragments::lazy;
use crate::multipass::extract_fields::ExtractFields;

#[derive(Debug, PartialEq)]
pub struct Parsed<'a> {
    pub fields: Vec<lazy::Field<'a>>,
    pub body: &'a [u8],
}

impl<'a> From <ExtractFields<'a>> for Parsed<'a> {
    fn from(ef: ExtractFields<'a>) -> Self {
        Parsed {
            fields: ef.fields.iter().map(|e| (*e).into()).collect(),
            body: ef.body,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_name() {
        assert_eq!(Parsed::from(ExtractFields {
            fields: vec![
                "From: hello@world.com,\r\n\talice@wonderlands.com\r\n",
                "Date: 12 Mar 1997 07:33:25 Z\r\n",
            ],
            body: b"Hello world!",
        }),
        Parsed {
            fields: vec![
                lazy::Field::From(lazy::MailboxList("hello@world.com,\r\n\talice@wonderlands.com\r\n")),
                lazy::Field::Date(lazy::DateTime("12 Mar 1997 07:33:25 Z\r\n")),
            ],
            body: b"Hello world!",
        });
    }
}
