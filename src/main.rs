use nom::{
    IResult, 
    character::complete::alphanumeric1,
    bytes::complete::tag,
    bytes::complete::take_until1,
};

#[derive(Debug, PartialEq)]
pub struct HeaderField {
    pub name: String,
    pub body: String,
}

fn parse_header_field(input: &str) -> IResult<&str, HeaderField> {
    let (input, name) = alphanumeric1(input)?;
    let (input, _) = tag(": ")(input)?;
    let (input, body) = take_until1("\r\n")(input)?;
    Ok((input, HeaderField { name: name.to_string(), body: body.to_string() }))
}

fn main() {
    let header_fields = "Subject: Hello\r\n World";
    println!("{:?}", parse_header_field(header_fields));
}
