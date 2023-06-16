use chrono::{DateTime, FixedOffset};
use nom::IResult;
use crate::misc_token;

pub fn section(input: &str) -> IResult<&str, DateTime<FixedOffset>> {
    // @FIXME want to extract datetime our way in the future
    // to better handle obsolete/bad cases instead of returning raw text.
    let (input, raw_date) = misc_token::unstructured(input)?;
    Ok((input, DateTime::parse_from_rfc2822(&raw_date).unwrap()))
}
