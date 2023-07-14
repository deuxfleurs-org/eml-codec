use crate::fragments::part;
use crate::fragments::section::Section;
use crate::multipass::header_section;

#[derive(Debug, PartialEq)]
pub struct Parsed<'a> {
    pub fields: Section<'a>,
    pub body: part::PartNode<'a>,
}

pub fn new<'a>(p: &'a header_section::Parsed<'a>) -> Parsed<'a> {
    todo!();
    /*Parsed {
        fields: p.fields,
        body: p.body,
    }*/
}
