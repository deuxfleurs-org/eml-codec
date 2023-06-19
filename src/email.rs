#[derive(Debug, PartialEq)]
pub struct Raw<'a>(&'a [u8]);

pub struct Segment<'a> {
    pub header: &'a [u8],
    pub body: &'a [u8],
}

/*
pub struct DecodeHeader<'a> {
    pub header: Cow<'a, &str>;
    pub encoding: &'static Encoding;
    pub is_malformed: bool;
    pub body: &'a [u8];
}

pub struct ExtractHeaderLines<'a> {
    pub header: Vec<Cow<'a, &str>>;
    pub body: &'a [u8];
}

pub struct ParseFieldNames<'a> {
    pub header: Vec<FieldName>;
    pub body: &'a [u8];
    pub bad_lines: Vec<Cow<'a, &str>;
}

pub struct ParseFieldBody<'a> {
    pub header: Vec<FieldBody>;
    pub body: &'a [u8];
    pub bad_lines: Vec<Cow<'a, &str>;
    pub bad_fields: Vec<FieldName>;
}

pub struct BuildHeaderSection<'a> {
    pub header: Section<'a>;
    pub body: &'a [u8];
}*/

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_parser() {
        assert_eq!(
            Raw(b"From: a@a.com\r\n\r\n"),
            Raw(&[0x46, 0x72, 0x6F, 0x6D, 0x3A, 0x20, 0x61, 0x40, 0x61, 0x2E, 0x63, 0x6F, 0x6D, 0x0D, 0x0A, 0x0D, 0x0A]),
        );
    }
}
