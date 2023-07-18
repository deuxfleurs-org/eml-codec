use nom::{
    self,
    combinator::{all_consuming, recognize},
    multi::many0,
    sequence::terminated,
    IResult,
};

use crate::text::whitespace::{foldable_line, line, obs_crlf};

pub fn headers(input: &[u8]) -> IResult<&[u8], Vec<&[u8]>> {
    let (body, hdrs) = segment(input)?;
    let (_, fields) = fields(hdrs)?;
    Ok((body, fields))
}

// -- part 1, segment
fn segment(input: &[u8]) -> IResult<&[u8], &[u8]> {
    terminated(recognize(many0(line)), obs_crlf)(input)
}

// -- part 2, isolate fields
fn fields(input: &[u8]) -> IResult<&[u8], Vec<&[u8]>> {
    let (rest, parsed) = all_consuming(many0(foldable_line))(input)?;
    Ok((rest, parsed))
}

