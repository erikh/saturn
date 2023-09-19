use super::{EntryRecord, EntryState};
use crate::{
    record::{Record, RecurringRecord},
    time::now,
};
use anyhow::{anyhow, Result};
use chrono::{Datelike, Duration, Timelike};
use fancy_duration::FancyDuration;

const DATE_ENDINGS: [&str; 4] = ["th", "st", "rd", "nd"];

pub fn parse_entry(args: Vec<String>, use_24h_time: bool) -> Result<EntryRecord> {
    let mut record = Record::build();
    let mut state = EntryState::Date;

    let mut scheduled_first: Option<chrono::NaiveTime> = None;
    let mut recurrence: Option<FancyDuration<Duration>> = None;
    let mut today = false;

    for arg in &args {
        match state {
            EntryState::Recur => {
                recurrence = Some(FancyDuration::<Duration>::parse(arg)?);
                state = EntryState::Date;
            }
            EntryState::Date => {
                match arg.to_lowercase().as_str() {
                    "today" => {
                        record.set_date(now().date_naive());
                        if !use_24h_time {
                            today = true;
                        }
                        state = EntryState::Time;
                    }
                    "yesterday" => {
                        record.set_date((now() - Duration::days(1)).date_naive());
                        state = EntryState::Time;
                    }
                    "tomorrow" => {
                        record.set_date((now() + Duration::days(1)).date_naive());
                        state = EntryState::Time;
                    }
                    "recur" => {
                        state = EntryState::Recur;
                    }
                    _ => {
                        record.set_date(parse_date(arg.to_string())?);
                        state = EntryState::Time;
                    }
                };
            }
            EntryState::Time => match arg.as_str() {
                "all" => {
                    record.set_all_day();
                    state = EntryState::TimeAt
                }
                "at" => state = EntryState::TimeAt,
                "from" => state = EntryState::TimeScheduled,
                _ => return Err(anyhow!("Time must be 'from' or 'at'")),
            },
            EntryState::TimeAt => {
                if arg != "day" {
                    record.set_at(Some(parse_time(arg.to_string(), today)?));
                }
                state = EntryState::Notify;
            }
            EntryState::TimeScheduled => {
                scheduled_first = Some(parse_time(arg.to_string(), today)?);
                state = EntryState::TimeScheduledHalf;
            }
            EntryState::TimeScheduledHalf => match arg.as_str() {
                "to" | "until" => {}
                _ => {
                    record.set_scheduled(Some((
                        scheduled_first.unwrap(),
                        parse_time(arg.to_string(), today)?,
                    )));
                    state = EntryState::Notify;
                }
            },
            EntryState::Notify => match arg.as_str() {
                "notify" => state = EntryState::NotifyTime,
                _ => {
                    record.set_detail(arg.to_string());
                    state = EntryState::Detail;
                }
            },
            EntryState::NotifyTime => match arg.as_str() {
                "me" => {}
                _ => {
                    let duration = FancyDuration::<Duration>::parse(arg)?;
                    if let Some(at) = record.at() {
                        record.add_notification(at - duration.duration());
                    } else if let Some(scheduled) = record.scheduled() {
                        record.add_notification(scheduled.0 - duration.duration());
                    } else if record.all_day() {
                        record.add_notification(time(0, 0) - duration.duration());
                    } else {
                        return Err(anyhow!(
                            "No time was scheduled to base this notification off of"
                        ));
                    }
                    state = EntryState::Detail;
                }
            },
            EntryState::Detail => {
                if record.detail().is_empty() {
                    record.set_detail(arg.to_string());
                } else {
                    record.set_detail(format!("{} {}", record.detail(), arg));
                }
            }
        }
    }

    Ok(EntryRecord {
        record: record.clone(),
        recurrence: recurrence.map_or_else(|| None, |x| Some(RecurringRecord::new(record, x))),
    })
}

fn parse_date(s: String) -> Result<chrono::NaiveDate> {
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

fn parse_time(s: String, today: bool) -> Result<chrono::NaiveTime> {
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
            ("8:00", chrono::NaiveTime::from_hms_opt(8, 0, 0).unwrap()),
            ("8am", chrono::NaiveTime::from_hms_opt(8, 0, 0).unwrap()),
            ("8:00pm", chrono::NaiveTime::from_hms_opt(20, 0, 0).unwrap()),
            ("8pm", chrono::NaiveTime::from_hms_opt(20, 0, 0).unwrap()),
            (
                "8:30pm",
                chrono::NaiveTime::from_hms_opt(20, 30, 0).unwrap(),
            ),
            ("8", chrono::NaiveTime::from_hms_opt(8, 0, 0).unwrap()),
            ("8:30", chrono::NaiveTime::from_hms_opt(8, 30, 0).unwrap()),
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

    #[test]
    fn test_parse_entry() {
        use super::parse_entry;
        use crate::{record::Record, time::now};
        use chrono::{Datelike, Duration, Timelike};

        let pm = now().hour() >= 12;
        let record = Record::build();

        let mut today = record.clone();
        today
            .set_date(chrono::Local::now().naive_local().date())
            .set_at(Some(
                chrono::NaiveTime::from_hms_opt(if pm { 20 } else { 8 }, 0, 0).unwrap(),
            ))
            .add_notification(
                chrono::NaiveTime::from_hms_opt(if pm { 19 } else { 7 }, 55, 0).unwrap(),
            )
            .set_detail("Test Today".to_string());

        let mut soda = record.clone();
        soda.set_date(chrono::NaiveDate::from_ymd_opt(now().year(), 8, 5).unwrap())
            .set_at(Some(chrono::NaiveTime::from_hms_opt(8, 0, 0).unwrap()))
            .add_notification(chrono::NaiveTime::from_hms_opt(7, 55, 0).unwrap())
            .set_detail("Get a Soda".to_string());

        let mut relax = record.clone();
        relax
            .set_date((now() + Duration::days(1)).date_naive())
            .set_at(Some(chrono::NaiveTime::from_hms_opt(16, 0, 0).unwrap()))
            .set_detail("Relax".to_string());

        let mut birthday = record.clone();
        birthday
            .set_date(chrono::NaiveDate::from_ymd_opt(now().year(), 10, 23).unwrap())
            .set_at(Some(chrono::NaiveTime::from_hms_opt(7, 30, 0).unwrap()))
            .add_notification(chrono::NaiveTime::from_hms_opt(6, 30, 0).unwrap())
            .set_detail("Tell my daughter 'happy birthday'".to_string());

        let mut new_year = record.clone();
        new_year
            .set_date(chrono::NaiveDate::from_ymd_opt(now().year(), 1, 1).unwrap())
            .set_at(Some(chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap()))
            .set_detail("Happy new year!".to_string());

        let mut christmas = record.clone();
        christmas
            .set_date(chrono::NaiveDate::from_ymd_opt(now().year(), 12, 25).unwrap())
            .set_scheduled(Some((
                chrono::NaiveTime::from_hms_opt(7, 0, 0).unwrap(),
                chrono::NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
            )))
            .set_detail("Christmas Morning".to_string());

        let table = vec![
            ("today at 8 notify me 5m Test Today", today),
            ("08/05 at 8 notify me 5m Get a Soda", soda),
            ("tomorrow at 4pm Relax", relax),
            (
                "10/23 at 7:30am notify 1h Tell my daughter 'happy birthday'",
                birthday,
            ),
            ("1/1 at 12am Happy new year!", new_year),
            ("12/25 from 7am to 12pm Christmas Morning", christmas),
        ];

        for (to_parse, t) in table {
            assert_eq!(
                parse_entry(
                    to_parse
                        .split(" ")
                        .map(|s| s.to_string())
                        .collect::<Vec<String>>(),
                    false,
                )
                .unwrap()
                .record,
                t,
            )
        }
    }
}
