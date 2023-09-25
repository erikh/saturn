use crate::time::now;
use anyhow::{anyhow, Result};
use chrono::{Datelike, Timelike};

const DATE_ENDINGS: [&str; 4] = ["th", "st", "rd", "nd"];

pub fn parse_date(s: String) -> Result<chrono::NaiveDate> {
    let regex = regex::Regex::new(r#"[/.-]"#)?;
    let split = regex.split(&s);
    let parts = split.collect::<Vec<&str>>();
    match parts.len() {
        3 => {
            // FIXME this should be locale-based
            Ok(chrono::NaiveDate::from_ymd_opt(
                parts[0].parse()?,
                parts[1].parse()?,
                parts[2].parse()?,
            )
            .expect("Invalid Date"))
        }
        2 => {
            // FIXME this should be locale-based
            Ok(
                chrono::NaiveDate::from_ymd_opt(now().year(), parts[0].parse()?, parts[1].parse()?)
                    .expect("Invalid Date"),
            )
        }
        1 => {
            let now = now();
            let mut part = parts[0].trim().to_string();
            for ending in DATE_ENDINGS {
                if part.ends_with(ending) {
                    part = part.replace(ending, "");
                    break;
                }
            }
            // FIXME this should be locale-based
            Ok(
                chrono::NaiveDate::from_ymd_opt(now.year(), now.month(), part.parse()?)
                    .expect("Invalid Date"),
            )
        }
        _ => Err(anyhow!("Cannot parse date")),
    }
}

fn twelve_hour_time(pm: bool, hour: u32, minute: u32) -> chrono::NaiveTime {
    let new_hour = if pm { 12 } else { 0 };

    time(
        if hour > 12 {
            hour
        } else if hour == 12 {
            new_hour
        } else {
            hour + new_hour
        },
        minute,
    )
}

fn time(hour: u32, minute: u32) -> chrono::NaiveTime {
    chrono::NaiveTime::from_hms_opt(hour, minute, 0).expect("Invalid Time")
}

fn pm_time(hour: u32, minute: u32) -> chrono::NaiveTime {
    twelve_hour_time(true, hour, minute)
}

fn am_time(hour: u32, minute: u32) -> chrono::NaiveTime {
    twelve_hour_time(false, hour, minute)
}

fn time_period(hour: u32, minute: u32, today: bool) -> chrono::NaiveTime {
    if today {
        if now().hour() >= 12 {
            pm_time(hour, minute)
        } else {
            am_time(hour, minute)
        }
    } else {
        time(hour, minute)
    }
}

fn designation(
    hour: u32,
    minute: u32,
    designation: &str,
    today: bool,
) -> Result<chrono::NaiveTime> {
    match designation {
        "pm" | "PM" => Ok(pm_time(hour, minute)),
        "am" | "AM" => Ok(am_time(hour, minute)),
        "" => Ok(time_period(hour, minute, today)),
        _ => Err(anyhow!("Cannot parse time")),
    }
}

pub fn parse_time(s: String, today: bool) -> Result<chrono::NaiveTime> {
    let s = s.trim();

    match s.to_lowercase().as_str() {
        "midnight" => return Ok(time(0, 0)),
        "noon" => return Ok(time(12, 0)),
        _ => {}
    }

    let regex = regex::Regex::new(r#"[:.]"#)?;
    let split = regex.split(s);
    let parts = split.collect::<Vec<&str>>();

    match parts.len() {
        3 => Ok(chrono::NaiveTime::from_hms_opt(
            parts[0].parse()?,
            parts[1].parse()?,
            parts[2].parse()?,
        )
        .expect("Invalid Time")),
        2 => {
            let regex = regex::Regex::new(r"(\d+)(\D+)")?;
            if let Some(captures) = regex.captures(parts[1]) {
                let hour: u32 = parts[0].parse()?;

                let minute: u32 = if let Some(minute) = captures.get(1) {
                    minute.as_str().parse()?
                } else {
                    return Err(anyhow!("Cannot parse time"));
                };

                if let Some(d) = captures.get(2) {
                    designation(hour, minute, d.as_str(), today)
                } else {
                    Ok(time_period(hour, minute, today))
                }
            } else {
                let hour: u32 = parts[0].parse()?;
                let minute: u32 = parts[1].parse()?;

                Ok(time_period(hour, minute, today))
            }
        }
        1 => {
            let regex = regex::Regex::new(r"(\d+)(\D*)")?;
            if let Some(captures) = regex.captures(parts[0]) {
                let hour: u32 = if let Some(hour) = captures.get(1) {
                    hour.as_str().parse()?
                } else {
                    return Err(anyhow!("Cannot parse time"));
                };

                if let Some(d) = captures.get(2) {
                    designation(hour, 0, d.as_str(), today)
                } else {
                    Ok(time_period(hour, 0, today))
                }
            } else {
                Err(anyhow!("Cannot parse time"))
            }
        }
        _ => Err(anyhow!("Cannot parse time")),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_parse_date() {
        use super::parse_date;
        use crate::time::now;
        use chrono::Datelike;

        let table = vec![
            (
                "2018-10-23",
                chrono::NaiveDate::from_ymd_opt(2018, 10, 23).unwrap(),
            ),
            (
                "2018/10/23",
                chrono::NaiveDate::from_ymd_opt(2018, 10, 23).unwrap(),
            ),
            (
                "2018.10.23",
                chrono::NaiveDate::from_ymd_opt(2018, 10, 23).unwrap(),
            ),
            (
                "10.23",
                chrono::NaiveDate::from_ymd_opt(now().year(), 10, 23).unwrap(),
            ),
            (
                "10/23",
                chrono::NaiveDate::from_ymd_opt(now().year(), 10, 23).unwrap(),
            ),
            (
                "10-23",
                chrono::NaiveDate::from_ymd_opt(now().year(), 10, 23).unwrap(),
            ),
            (
                "23",
                chrono::NaiveDate::from_ymd_opt(now().year(), now().month(), 23).unwrap(),
            ),
        ];

        for (to_parse, t) in table {
            assert_eq!(parse_date(to_parse.to_string()).unwrap(), t)
        }
    }

    #[test]
    fn test_parse_time() {
        use super::parse_time;
        use crate::time::now;
        use chrono::Timelike;

        let pm = now().hour() >= 12;

        let today_table = vec![
            ("12am", chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap()),
            ("12pm", chrono::NaiveTime::from_hms_opt(12, 0, 0).unwrap()),
            ("8:00:00", chrono::NaiveTime::from_hms_opt(8, 0, 0).unwrap()),
            (
                "8:12:56",
                chrono::NaiveTime::from_hms_opt(8, 12, 56).unwrap(),
            ),
            (
                "8:00",
                chrono::NaiveTime::from_hms_opt(if pm { 20 } else { 8 }, 0, 0).unwrap(),
            ),
            ("8am", chrono::NaiveTime::from_hms_opt(8, 0, 0).unwrap()),
            ("8:00pm", chrono::NaiveTime::from_hms_opt(20, 0, 0).unwrap()),
            ("8pm", chrono::NaiveTime::from_hms_opt(20, 0, 0).unwrap()),
            (
                "8:30pm",
                chrono::NaiveTime::from_hms_opt(20, 30, 0).unwrap(),
            ),
            (
                "8",
                chrono::NaiveTime::from_hms_opt(if pm { 20 } else { 8 }, 0, 0).unwrap(),
            ),
            (
                "8:30",
                chrono::NaiveTime::from_hms_opt(if pm { 20 } else { 8 }, 30, 0).unwrap(),
            ),
        ];

        let other_table = vec![
            ("12am", chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap()),
            ("12pm", chrono::NaiveTime::from_hms_opt(12, 0, 0).unwrap()),
            ("8:00:00", chrono::NaiveTime::from_hms_opt(8, 0, 0).unwrap()),
            (
                "8:12:56",
                chrono::NaiveTime::from_hms_opt(8, 12, 56).unwrap(),
            ),
            (
                "8:00",
                chrono::NaiveTime::from_hms_opt(if pm { 20 } else { 8 }, 0, 0).unwrap(),
            ),
            ("8am", chrono::NaiveTime::from_hms_opt(8, 0, 0).unwrap()),
            ("8:00pm", chrono::NaiveTime::from_hms_opt(20, 0, 0).unwrap()),
            ("8pm", chrono::NaiveTime::from_hms_opt(20, 0, 0).unwrap()),
            (
                "8:30pm",
                chrono::NaiveTime::from_hms_opt(20, 30, 0).unwrap(),
            ),
            (
                "8",
                chrono::NaiveTime::from_hms_opt(if pm { 20 } else { 8 }, 0, 0).unwrap(),
            ),
            (
                "8:30",
                chrono::NaiveTime::from_hms_opt(if pm { 20 } else { 8 }, 30, 0).unwrap(),
            ),
        ];

        for (to_parse, t) in today_table {
            assert_eq!(
                parse_time(to_parse.to_string(), true).unwrap(),
                t,
                "{}",
                to_parse
            )
        }

        for (to_parse, t) in other_table {
            assert_eq!(
                parse_time(to_parse.to_string(), true).unwrap(),
                t,
                "{}",
                to_parse
            )
        }
    }
}
