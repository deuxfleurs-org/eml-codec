use crate::fragments::eager;
use crate::multipass::field_lazy;
use crate::multipass::header_section;

#[derive(Debug, PartialEq)]
pub struct Parsed<'a> {
    pub fields: Vec<eager::Field<'a>>,
    pub body: &'a [u8],
}

pub fn new<'a>(p: &'a field_lazy::Parsed<'a>) -> Parsed<'a> {
    Parsed {
        fields: p.fields
            .iter()
            .filter_map(|entry| entry.try_into().ok())
            .collect(),
        body: p.body,
    }
}

impl<'a> Parsed<'a> {
    pub fn section(&'a self) -> header_section::Parsed<'a> {
        header_section::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fragments::lazy;
    use crate::fragments::model;
    use chrono::{FixedOffset, TimeZone};

    #[test]
    fn test_field_body() {
        assert_eq!(new(&field_lazy::Parsed {
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
