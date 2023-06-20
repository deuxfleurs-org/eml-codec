use crate::fragments::field_raw;
use crate::multipass::extract_fields::ExtractFields;

#[derive(Debug, PartialEq)]
pub struct ParseFieldName<'a> {
    pub fields: Vec<field_raw::Field<'a>>,
    pub body: &'a [u8],
}

impl<'a> From <&'a ExtractFields<'a>> for ParseFieldName<'a> {
    fn from(ef: &'a ExtractFields<'a>) -> Self {
        ParseFieldName {
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
        assert_eq!(ParseFieldName::from(&ExtractFields {
            fields: vec![
                "From: hello@world.com,\r\n\talice@wonderlands.com\r\n",
                "Date: 12 Mar 1997 07:33:25 Z\r\n",
            ],
            body: b"Hello world!",
        }),
        ParseFieldName {
            fields: vec![
                field_raw::Field::From("hello@world.com,\r\n\talice@wonderlands.com\r\n"),
                field_raw::Field::Date("12 Mar 1997 07:33:25 Z\r\n"),
            ],
            body: b"Hello world!",
        });
    }
}
