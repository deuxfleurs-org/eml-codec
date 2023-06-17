use chrono::{DateTime, FixedOffset, NaiveDate, NaiveTime, TimeZone};
use nom::{
    IResult,
    bytes::complete::take_while_m_n,
    character::is_digit,
};
use crate::misc_token;

///  date-time       =   [ day-of-week "," ] date time [CFWS]
///  time            =   time-of-day zone
///  @FIXME: if parsing fails, Option::None is silently returned...
pub fn section(input: &str) -> IResult<&str, Option<DateTime<FixedOffset>>> {
    let (input, (_, date, time, tz, _)) = tuple((
        opt(terminated(day_of_week, tag(","))),
        date, time_of_day, zone
        opt(cfws)))(input)?;


    //@TODO: rebuild DateTime from NaiveDate, NaiveTime and TimeZone



    // @FIXME want to extract datetime our way in the future
    // to better handle obsolete/bad cases instead of returning raw text.
    //let (input, raw_date) = misc_token::unstructured(input)?;
    //Ok((input, DateTime::parse_from_rfc2822(&raw_date).unwrap()))
}

///    day-of-week     =   ([FWS] day-name) / obs-day-of-week
fn day_of_week(input: &str) -> IResult<&str, &str> {
    alt((day_of_week_strict, obs_day_of_week))(input)
}

fn day_of_week_strict(input: &str) -> IResult<&str, &str> {
    preceded(opt(fws), day_name)(input)
}

fn obs_day_of_week(input: &str) -> IResult<&str, &str> {
    delimited(obs(cfws), day_name, obs(cfws))(input)
}

///   day-name        =   "Mon" / "Tue" / "Wed" / "Thu" /
///                       "Fri" / "Sat" / "Sun"

fn day_name(input: &str) -> IResult<&str, &str> {
    alt((
        tag_no_case("Mon"),
        tag_no_case("Tue"),
        tag_no_case("Wed"),
        tag_no_case("Thu"),
        tag_no_case("Fri"),
        tag_no_case("Sat"),
        tag_no_case("Sun"),
    ))(input)
}

///    date            =   day month year
fn date(input: &str) -> IResult<&str, Option<NaiveDate>> {
   map(
       tuple((day, month, year)),
       |(d, m, y)| NaiveDate::from_ymd_opt(y, m, d)
    )(input)
}

///    day             =   ([FWS] 1*2DIGIT FWS) / obs-day
///    obs-day         =   [CFWS] 1*2DIGIT [CFWS]
fn day(input: &str) -> IResult<&str, u32> {
    alt((day_strict, obs_day))(input)
}

fn day_strict(input: &str) -> IResult<&str, u32) {
    delimited(opt(fws), day_digit, fws)(input)
}

fn obs_day(input: &str) -> IResult<&str, u32) {
    delimited(opt(cfws), day_digit, opt(cfws))(input)
}

fn day_digit(input: &str) -> IRresult<&str, u32) {
    map(take_while_m_n(1, 2, is_digit), |d| d.parse::<u32>().unwrap())(input)
}

///  month           =   "Jan" / "Feb" / "Mar" / "Apr" /
///                      "May" / "Jun" / "Jul" / "Aug" /
///                      "Sep" / "Oct" / "Nov" / "Dec"
fn month(input: &str) -> IResult<&str, u32) {
    alt((
        value(1, tag_no_case("Jan")),
        value(2, tag_no_case("Feb")),
        value(3, tag_no_case("Mar")),
        value(4, tag_no_case("Apr")),
        value(5, tag_no_case("May")),
        value(6, tag_no_case("Jun")),
        value(7, tag_no_case("Jul")),
        value(8, tag_no_case("Aug")),
        value(9, tag_no_case("Sep")),
        value(10, tag_no_case("Oct")),
        value(11, tag_no_case("Nov")),
        value(12, tag_no_case("Dec")),
    ))(input)
}

///   year            =   (FWS 4*DIGIT FWS) / obs-year
///   obs-year        =   [CFWS] 2*DIGIT [CFWS]
fn year(input: &str) -> IResult<&str, i32> {
    alt((strict_year, obs_year))(input)
}

fn strict_year(input &str) -> IResult<&str, i32> {
    delimited(fws, strict_year_digit, fws)(input)
}

fn obs_year(input: &str) -> IResult<&str, i32> {
    delimited(opt(cfws), obs_year_digit, opt(cfws))(input)
}

fn strict_year_digit(input: &str) -> IResult<&str, i32> {
    // Max value for i32 is 2,147,483,647 ; in other words 10 digits.
    // 9 digits should always be parsable into an i32 and enough for a year.
    // @FIXME a better implementation is desirable
    map(take_while_m_n(4, 9, is_digit), |d| d.parse::<i32>().unwrap())(input)
}

fn obs_year_digit(input: &str) -> IResult<&str, i32> {
    // @FIXME same as strict_year_digit
    map(take_while_m_n(2, 9, is_digit), |d| d.parse::<i32>().unwrap())(input)
}

///   time-of-day     =   hour ":" minute [ ":" second ]
///
fn time(input: &str) -> IResult<&str, (NaiveTime, TimeZone)> {
    map(
        tuple((time_digit, tag(":"), time_digit, opt(preceded(tag(":"), time_digit)))),
        |(hour, _, minute, maybe_sec)| 
}

fn time_digit(input: &str) -> IResult<&str, u32> {
    alt((strict_time_digit, obs_time_digit))(input)
}

fn strict_time_digit(input: &str) -> IResult<&str, u32> {
    take_while_m_n(4, 4, is_digit)(input)
}

