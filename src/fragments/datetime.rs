use chrono::{DateTime, FixedOffset, NaiveDate, NaiveTime, TimeZone};
use nom::{
    IResult,
    AsChar,
    branch::alt,
    bytes::complete::{tag, tag_no_case, take_while, take_while1, take_while_m_n, is_a},
    character,
    character::is_digit,
    character::complete::{one_of, alphanumeric1, digit0},
    combinator::{map, opt, value},
    sequence::{preceded, terminated, tuple, delimited },
};
use crate::fragments::misc_token;
use crate::fragments::lazy;
use crate::fragments::whitespace::{fws, cfws};
use crate::error::IMFError;

const MIN: i32 = 60;
const HOUR: i32 = 60 * MIN;

impl<'a> TryFrom<&'a lazy::DateTime<'a>> for DateTime<FixedOffset> {
    type Error = IMFError<'a>;

    fn try_from(value: &'a lazy::DateTime<'a>) -> Result<Self, Self::Error> {
        match section(value.0) {
            Ok((_, Some(dt))) => Ok(dt),
            Err(e) => Err(IMFError::DateTimeParse(e)),
            _ => Err(IMFError::DateTimeLogic),
        }
    }
}

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
    map(terminated(
            alt((
                tuple((opt(terminated(strict_day_of_week, tag(","))), strict_date, strict_time_of_day, strict_zone )),
                tuple((opt(terminated(obs_day_of_week, tag(","))), obs_date, obs_time_of_day, alt((strict_zone, obs_zone)) )),
            )),
            opt(cfws)
        ), |res| {
            match res {
                (_, Some(date), Some(time), Some(tz)) => {
                    date.and_time(time).and_local_timezone(tz).earliest()
                },
                _ => None,
            }
        })(input)
}

///    day-of-week     =   ([FWS] day-name) / obs-day-of-week
fn strict_day_of_week(input: &str) -> IResult<&str, &str> {
    preceded(opt(fws), day_name)(input)
}

///    obs-day-of-week =   [CFWS] day-name [CFWS]
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
fn strict_date(input: &str) -> IResult<&str, Option<NaiveDate>> {
   map(
       tuple((strict_day, month, strict_year)),
       |(d, m, y)| NaiveDate::from_ymd_opt(y, m, d)
    )(input)
}

///    date            =   day month year
fn obs_date(input: &str) -> IResult<&str, Option<NaiveDate>> {
   map(
       tuple((obs_day, month, obs_year)),
       |(d, m, y)| NaiveDate::from_ymd_opt(y, m, d)
    )(input)
}

///    day             =   ([FWS] 1*2DIGIT FWS) / obs-day
fn strict_day(input: &str) -> IResult<&str, u32> {
    delimited(opt(fws), character::complete::u32, fws)(input)
}

///    obs-day         =   [CFWS] 1*2DIGIT [CFWS]
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
fn strict_year(input: &str) -> IResult<&str, i32> {
    delimited(
        fws, 
        map(
            terminated(take_while_m_n(4,9,|c| c >= '\x30' && c <= '\x39'), digit0), 
            |d: &str| d.parse::<i32>().unwrap()), 
        fws,
    )(input)
}

///   obs-year        =   [CFWS] 2*DIGIT [CFWS]
fn obs_year(input: &str) -> IResult<&str, i32> {
    map(delimited(
        opt(cfws), 
        terminated(take_while_m_n(2,7,|c| c >= '\x30' && c <= '\x39'), digit0), 
        opt(cfws)
    ), |cap: &str| {
        let d = cap.parse::<i32>().unwrap();
        if d >= 0 && d <= 49 {
            2000 + d
        } else if d >= 50 && d <= 999 {
            1900 + d
        } else {
            d
        }
    })(input)
}

///   time-of-day     =   hour ":" minute [ ":" second ]
fn strict_time_of_day(input: &str) -> IResult<&str, Option<NaiveTime>> {
    map(
        tuple((strict_time_digit, tag(":"), strict_time_digit, opt(preceded(tag(":"), strict_time_digit)))),
        |(hour, _, minute, maybe_sec)| NaiveTime::from_hms_opt(hour, minute, maybe_sec.unwrap_or(0)),
    )(input)
}

///   time-of-day     =   hour ":" minute [ ":" second ]
fn obs_time_of_day(input: &str) -> IResult<&str, Option<NaiveTime>> {
    map(
        tuple((obs_time_digit, tag(":"), obs_time_digit, opt(preceded(tag(":"), obs_time_digit)))),
        |(hour, _, minute, maybe_sec)| NaiveTime::from_hms_opt(hour, minute, maybe_sec.unwrap_or(0)),
    )(input)
}

fn strict_time_digit(input: &str) -> IResult<&str, u32> {
    character::complete::u32(input)
}

fn obs_time_digit(input: &str) -> IResult<&str, u32> {
    delimited(opt(cfws), character::complete::u32, opt(cfws))(input)
}

/// Obsolete zones
///
/// ```abnf
///   zone            =   (FWS ( "+" / "-" ) 4DIGIT) / (FWS obs-zone)
/// ```
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

/// obsole zone
///
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
fn obs_zone(input: &str) -> IResult<&str, Option<FixedOffset>> {
    // The writing of this function is volontarily verbose
    // to keep it straightforward to understand.
    // @FIXME: Could return a TimeZone and not an Option<TimeZone>
    // as it could be determined at compile time if values are correct
    // and panic at this time if not. But not sure how to do it without unwrap.
    preceded(
        opt(fws),
        alt((
            // Legacy UTC/GMT
            value(FixedOffset::west_opt(0 * HOUR), alt((tag("UTC"), tag("UT"), tag("GMT")))),

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
        )),
    )(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    
    #[test]
    fn test_section_rfc_strict() {
        assert_eq!(
            section("Fri, 21 Nov 1997 09:55:06 -0600"), 
            Ok(("", Some(FixedOffset::west_opt(6 * HOUR).unwrap().with_ymd_and_hms(1997, 11, 21, 9, 55, 6).unwrap()))),
        );
    }

    #[test]
    fn test_section_received() {
        assert_eq!(
            section("Sun, 18 Jun 2023 15:39:08 +0200 (CEST)"),
            Ok(("", Some(FixedOffset::east_opt(2 * HOUR).unwrap().with_ymd_and_hms(2023, 6, 18, 15, 39, 8).unwrap()))),
        );
    }

    #[test]
    fn test_section_rfc_ws() {
        assert_eq!(
            section(
                r#"Thu,
         13
           Feb
             1969
         23:32
                  -0330 (Newfoundland Time)"#),
            Ok(("", Some(FixedOffset::west_opt(3 * HOUR + 30 * MIN).unwrap().with_ymd_and_hms(1969, 2, 13, 23, 32, 00).unwrap()))),
        );
    }

    #[test]
    fn test_section_rfc_obs() {
        assert_eq!(
            section("21 Nov 97 09:55:06 GMT"),
            Ok(("", Some(FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(1997, 11, 21, 9, 55, 6).unwrap()))),
        );
    }

    #[test]
    fn test_section_3digit_year() {
        assert_eq!(
            section("21 Nov 103 09:55:06 UT"),
            Ok(("", Some(FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(2003, 11, 21, 9, 55, 6).unwrap()))),
        );
    }

    #[test]
    fn test_section_rfc_obs_ws() {
        assert_eq!(
            section("Fri, 21 Nov 1997 09(comment):   55  :  06 -0600"),
            Ok(("", Some(FixedOffset::west_opt(6 * HOUR).unwrap().with_ymd_and_hms(1997, 11, 21, 9, 55, 6).unwrap()))),
        );
    }

    #[test]
    fn test_section_2digit_year() {
        assert_eq!(
            section("21 Nov 23 09:55:06Z"),
            Ok(("", Some(FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(2023, 11, 21, 9, 55, 6).unwrap()))),
        );
    }

    #[test]
    fn test_section_military_zone_east() {
        ["a", "B", "c", "D", "e", "F", "g", "H", "i", "K", "l", "M"].iter().enumerate().for_each(|(i, x)| {
            assert_eq!(
                section(format!("1 Jan 22 08:00:00 {}", x).as_str()),
                Ok(("", Some(FixedOffset::east_opt((i as i32 + 1) * HOUR).unwrap().with_ymd_and_hms(2022, 01, 01, 8, 0, 0).unwrap())))
            );
        });
    }

    #[test]
    fn test_section_military_zone_west() {
        ["N", "O", "P", "q", "r", "s", "T", "U", "V", "w", "x", "y"].iter().enumerate().for_each(|(i, x)| {
            assert_eq!(
                section(format!("1 Jan 22 08:00:00 {}", x).as_str()),
                Ok(("", Some(FixedOffset::west_opt((i as i32 + 1) * HOUR).unwrap().with_ymd_and_hms(2022, 01, 01, 8, 0, 0).unwrap())))
            );
        });
    }

    #[test]
    fn test_section_gmt() {
        assert_eq!(
            section("21 Nov 2023 07:07:07 +0000"),
            Ok(("", Some(FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(2023, 11, 21, 7, 7, 7).unwrap()))),
        );
        assert_eq!(
            section("21 Nov 2023 07:07:07 -0000"),
            Ok(("", Some(FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(2023, 11, 21, 7, 7, 7).unwrap()))),
        );
        assert_eq!(
            section("21 Nov 2023 07:07:07 Z"),
            Ok(("", Some(FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(2023, 11, 21, 7, 7, 7).unwrap()))),
        );
        assert_eq!(
            section("21 Nov 2023 07:07:07 GMT"),
            Ok(("", Some(FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(2023, 11, 21, 7, 7, 7).unwrap()))),
        );
        assert_eq!(
            section("21 Nov 2023 07:07:07 UT"),
            Ok(("", Some(FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(2023, 11, 21, 7, 7, 7).unwrap()))),
        );
        assert_eq!(
            section("21 Nov 2023 07:07:07 UTC"),
            Ok(("", Some(FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(2023, 11, 21, 7, 7, 7).unwrap()))),
        );
    }

    #[test]
    fn test_section_usa() {
        assert_eq!(
            section("21 Nov 2023 4:4:4 CST"),
            Ok(("", Some(FixedOffset::west_opt(6 * HOUR).unwrap().with_ymd_and_hms(2023, 11, 21, 4, 4, 4).unwrap()))),
        );
    }
}
