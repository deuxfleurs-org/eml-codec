use std::collections::HashMap;
use chrono::{DateTime,FixedOffset,ParseError};

#[derive(Debug, Default)]
pub enum HeaderDate {
    Parsed(DateTime<FixedOffset>),
    Unknown(String, ParseError),
    #[default]
    None,
}

#[derive(Debug, Default)]
pub struct HeaderSection<'a> {
    pub subject: Option<String>,
    pub from: Vec<String>,
    pub date: HeaderDate,
    pub optional: HashMap<&'a str, String>,
}

enum InvalidEmailErr {
    NoUsableDate,
}

impl<'a> HeaderSection<'a> {
    fn is_valid(&self) -> Result<(), InvalidEmailErr> {
        match self.date {
            HeaderDate::Parsed(_) => (),
            _ => return Err(InvalidEmailErr::NoUsableDate),
        };

        Ok(())
    }
}
