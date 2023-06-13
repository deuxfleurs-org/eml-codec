use nom::{
    IResult,
    multi::many0,
};

use crate::{common_fields, trace, whitespace, model};

pub fn section(input: &str) -> IResult<&str, model::HeaderSection> {
    let (input, traces) = many0(trace::section)(input)?;
    let (input, common) = common_fields::section(input)?;
    let (input, _) = whitespace::perm_crlf(input)?;

    Ok((input, model::HeaderSection { traces, common }))
}


