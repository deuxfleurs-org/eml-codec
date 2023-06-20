use crate::fragments::eager;
use crate::multipass::field_lazy;

#[derive(Debug, PartialEq)]
pub struct Parsed<'a> {
    pub fields: Vec<eager::Field<'a>>,
    pub body: &'a [u8],
}

impl<'a> From <&'a field_lazy::Parsed<'a>> for Parsed<'a> {
    fn from(p: &'a field_lazy::Parsed<'a>) -> Self {
        Parsed {
            fields: p.fields.iter().filter_map(|entry| entry.try_into().ok()).collect(),
            body: p.body,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fragments::lazy;
    use crate::fragments::model;
    use chrono::{FixedOffset, TimeZone};

    #[test]
    fn test_field_name() {
        assert_eq!(Parsed::from(&field_lazy::Parsed {
            fields: vec![
                lazy::Field::From(lazy::MailboxList("hello@world.com,\r\n\talice@wonderlands.com\r\n")),
                lazy::Field::Date(lazy::DateTime("12 Mar 1997 07:33:25 Z\r\n")),
            ],
            body: b"Hello world!",
        }),
        Parsed {
            fields: vec![
                eager::Field::From(vec![
                    model::MailboxRef { 
                        name: None, 
                        addrspec: model::AddrSpec { 
                            local_part: "hello".into(), 
                            domain: "world.com".into() 
                        }
                    }, 
                    model::MailboxRef { 
                        name: None, 
                        addrspec: model::AddrSpec { 
                            local_part: "alice".into(), 
                            domain: "wonderlands.com".into() 
                        }
                    },
                ]),
                eager::Field::Date(
                    FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(1997, 03, 12, 7, 33, 25)
                    .unwrap()
                ),
            ],
            body: b"Hello world!",
        });
    }
}
