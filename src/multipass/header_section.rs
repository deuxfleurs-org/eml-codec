use crate::fragments::section::Section;
use crate::multipass::field_eager;

#[derive(Debug, PartialEq)]
pub struct Parsed<'a> {
    pub fields: Section<'a>,
    pub body: &'a [u8],
}

pub fn new<'a>(p: &'a field_eager::Parsed<'a>) -> Parsed<'a> {
    Parsed {
        fields: Section::from_iter(p.fields.iter()),
        body: p.body,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fragments::eager;
    use crate::fragments::model;
    use chrono::{FixedOffset, TimeZone};

    #[test]
    fn test_section() {
        assert_eq!(new(&field_eager::Parsed {
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
        }),
        Parsed {
            fields: Section {
                from: vec![
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
                ],

                date: Some(FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(1997, 03, 12, 7, 33, 25)
                    .unwrap()),

                ..Default::default()
            },
            body: b"Hello world!",
        });
    }
}
