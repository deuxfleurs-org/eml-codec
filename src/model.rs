use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct HeaderSection<'a> {
    pub subject: Option<String>,
    pub from: Vec<String>,
    pub optional: HashMap<&'a str, String>,
}
