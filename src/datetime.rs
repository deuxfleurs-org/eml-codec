use chrono::{DateTime, FixedOffset, NaiveDate, NaiveTime, TimeZone};
use nom::{
    IResult,
    AsChar,
    branch::alt,
    bytes::complete::{tag, tag_no_case, take_while, take_while1, take_while_m_n, is_a},
    character,
    character::is_digit,
    character::complete::{one_of, alphanumeric1},
    combinator::{map, opt, value},
    sequence::{preceded, terminated, tuple, delimited },
};
use crate::misc_token;
use crate::whitespace::{fws, cfws};

const MIN: i32 = 60;
const HOUR: i32 = 60 * MIN;

/// Read datetime
///
/// ```abnf
/// date-time       =   [ day-of-week "," ] date time [CFWS]
/// time            =   time-of-day zone
/// ```
///
/// ## @FIXME - known bugs
///  
///   - if chrono fails, Option::None is silently returned instead of failing the parser
///   - `-0000` means NaiveDateTime, a date without a timezone
/// while this library interprets it as +0000 aka UTC.
///   - Obsolete military zones should be considered as NaiveTime
/// due to an error in RFC0822 but are interpreted as their respective
/// timezone according to the RFC5322 definition
pub fn section(input: &str) -> IResult<&str, Option<DateTime<FixedOffset>>> {
    map(tuple((
            opt(terminated(day_of_week, tag(","))),
            date, time_of_day, zone,
            opt(cfws)
        )), |res| {
            match res {
                (_, Some(date), Some(time), Some(tz), _) => {
                    date.and_time(time).and_local_timezone(tz).earliest()
                },
                _ => None,
            }
        })(input)
}

///    day-of-week     =   ([FWS] day-name) / obs-day-of-week
fn day_of_week(input: &str) -> IResult<&str, &str> {
    alt((day_of_week_strict, obs_day_of_week))(input)
}

fn day_of_week_strict(input: &str) -> IResult<&str, &str> {
    preceded(opt(fws), day_name)(input)
}

fn obs_day_of_week(input: &str) -> IResult<&str, &str> {
    delimited(opt(cfws), day_name, opt(cfws))(input)
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

fn day_strict(input: &str) -> IResult<&str, u32> {
    delimited(opt(fws), character::complete::u32, fws)(input)
}

fn obs_day(input: &str) -> IResult<&str, u32> {
    delimited(opt(cfws), character::complete::u32, opt(cfws))(input)
}

///  month           =   "Jan" / "Feb" / "Mar" / "Apr" /
///                      "May" / "Jun" / "Jul" / "Aug" /
///                      "Sep" / "Oct" / "Nov" / "Dec"
fn month(input: &str) -> IResult<&str, u32> {
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

fn strict_year(input: &str) -> IResult<&str, i32> {
    delimited(fws, character::complete::i32, fws)(input)
}

fn obs_year(input: &str) -> IResult<&str, i32> {
    map(delimited(opt(cfws), character::complete::i32, opt(cfws)),
      |d: i32| if d >= 0 && d <= 49 {
        2000 + d
      } else if d >= 50 && d <= 999 {
        1900 + d
      } else {
        d
      })(input)
}

///   time-of-day     =   hour ":" minute [ ":" second ]
///
fn time_of_day(input: &str) -> IResult<&str, Option<NaiveTime>> {
    map(
        tuple((character::complete::u32, tag(":"), character::complete::u32, opt(preceded(tag(":"), character::complete::u32)))),
        |(hour, _, minute, maybe_sec)| NaiveTime::from_hms_opt(hour, minute, maybe_sec.unwrap_or(0)),
    )(input)
}

/// Obsolete zones
///
/// ```abnf
///   zone            =   (FWS ( "+" / "-" ) 4DIGIT) / obs-zone
///   obs-zone        =   "UT" / "GMT" /     ; Universal Time
///                                          ; North American UT
///                                          ; offsets
///                       "EST" / "EDT" /    ; Eastern:  - 5/ - 4
///                       "CST" / "CDT" /    ; Central:  - 6/ - 5
///                       "MST" / "MDT" /    ; Mountain: - 7/ - 6
///                       "PST" / "PDT" /    ; Pacific:  - 8/ - 7
///                                          ;
///                       %d65-73 /          ; Military zones - "A"
///                       %d75-90 /          ; through "I" and "K"
///                       %d97-105 /         ; through "Z", both
///                       %d107-122 /        ; upper and lower case
///                                          ;
///                       1*(ALPHA / DIGIT)  ; Unknown legacy timezones 
/// ```
///
/// 

fn zone(input: &str) -> IResult<&str, Option<FixedOffset>> {
    alt((strict_zone, obs_zone))(input)
}

fn strict_zone(input: &str) -> IResult<&str, Option<FixedOffset>> {
    map(
        tuple((opt(fws), is_a("+-"), take_while_m_n(2,2,|c| c >= '\x30' && c <= '\x39'), take_while_m_n(2,2,|c| c >= '\x30' && c <= '\x39'))),
        |(_, op, dig_zone_hour, dig_zone_min)| {
            let zone_hour = dig_zone_hour.parse::<i32>().unwrap() * HOUR;
            let zone_min = dig_zone_min.parse::<i32>().unwrap() * MIN;
            match op {
                "+" => FixedOffset::east_opt(zone_hour + zone_min),
                "-" => FixedOffset::west_opt(zone_hour + zone_min),
                _ => unreachable!(),             }
        }
    )(input)
}

fn obs_zone(input: &str) -> IResult<&str, Option<FixedOffset>> {
   // The writing of this function is volontarily verbose
   // to keep it straightforward to understand.
   // @FIXME: Could return a TimeZone and not an Option<TimeZone>
   // as it could be determined at compile time if values are correct
   // and panic at this time if not. But not sure how to do it without unwrap.
   alt((
    // Legacy UTC/GMT
    value(FixedOffset::west_opt(0 * HOUR), alt((tag("UT"), tag("GMT")))),

    // USA Timezones
    value(FixedOffset::west_opt(4 * HOUR), tag("EDT")),
    value(FixedOffset::west_opt(5 * HOUR), alt((tag("EST"), tag("CDT")))),
    value(FixedOffset::west_opt(6 * HOUR), alt((tag("CST"), tag("MDT")))),
    value(FixedOffset::west_opt(7 * HOUR), alt((tag("MST"), tag("PDT")))),
    value(FixedOffset::west_opt(8 * HOUR), tag("PST")),

    // Military Timezone UTC
    value(FixedOffset::west_opt(0 * HOUR), tag("Z")),

    // Military Timezones East
    map(one_of("ABCDEFGHIKLMabcdefghiklm"), |c| match c {
        'A' | 'a' => FixedOffset::east_opt(1 * HOUR),
        'B' | 'b' => FixedOffset::east_opt(2 * HOUR),
        'C' | 'c' => FixedOffset::east_opt(3 * HOUR),
        'D' | 'd' => FixedOffset::east_opt(4 * HOUR),
        'E' | 'e' => FixedOffset::east_opt(5 * HOUR),
        'F' | 'f' => FixedOffset::east_opt(6 * HOUR),
        'G' | 'g' => FixedOffset::east_opt(7 * HOUR),
        'H' | 'h' => FixedOffset::east_opt(8 * HOUR),
        'I' | 'i' => FixedOffset::east_opt(9 * HOUR),
        'K' | 'k' => FixedOffset::east_opt(10 * HOUR),
        'L' | 'l' => FixedOffset::east_opt(11 * HOUR),
        'M' | 'm' => FixedOffset::east_opt(12 * HOUR),
        _ => unreachable!(),
    }),

    // Military Timezones West
    map(one_of("nopqrstuvwxyNOPQRSTUVWXY"), |c| match c {
        'N' | 'n' => FixedOffset::west_opt(1 * HOUR),
        'O' | 'o' => FixedOffset::west_opt(2 * HOUR),
        'P' | 'p' => FixedOffset::west_opt(3 * HOUR),
        'Q' | 'q' => FixedOffset::west_opt(4 * HOUR),
        'R' | 'r' => FixedOffset::west_opt(5 * HOUR),
        'S' | 's' => FixedOffset::west_opt(6 * HOUR),
        'T' | 't' => FixedOffset::west_opt(7 * HOUR),
        'U' | 'u' => FixedOffset::west_opt(8 * HOUR),
        'V' | 'v' => FixedOffset::west_opt(9 * HOUR),
        'W' | 'w' => FixedOffset::west_opt(10 * HOUR),
        'X' | 'x' => FixedOffset::west_opt(11 * HOUR),
        'Y' | 'y' => FixedOffset::west_opt(12 * HOUR),
        _ => unreachable!(),
    }),

    // Unknown timezone
    value(FixedOffset::west_opt(0 * HOUR), alphanumeric1),
  ))(input)
}
