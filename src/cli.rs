#![allow(dead_code)]
use crate::{db::DB, record::Record};
use anyhow::anyhow;
use std::{env::var, path::PathBuf};

pub fn saturn_db() -> PathBuf {
    PathBuf::from(
        var("SATURN_DB").unwrap_or(
            PathBuf::from(var("HOME").unwrap_or("/".to_string()))
                .join(".saturn.db")
                .to_str()
                .unwrap()
                .to_string(),
        ),
    )
}

pub struct EntryParser {
    #[allow(dead_code)]
    args: Vec<String>,
    filename: PathBuf,
}

impl EntryParser {
    pub fn new(args: Vec<String>) -> Self {
        Self {
            args,
            filename: saturn_db(),
        }
    }

    pub fn entry(&self) -> Result<(), anyhow::Error> {
        let mut db = if std::fs::metadata(&self.filename).is_ok() {
            DB::load(self.filename.clone())?
        } else {
            DB::default()
        };

        db.record(self.to_record()?);
        db.dump(self.filename.clone())?;

        Ok(())
    }

    pub fn to_record(&self) -> Result<Record, anyhow::Error> {
        parse_entry(self.args.clone())
    }
}

pub fn list_entries() -> Result<Vec<Record>, anyhow::Error> {
    let filename = saturn_db();

    let db = if std::fs::metadata(&filename).is_ok() {
        DB::load(filename.clone())?
    } else {
        DB::default()
    };

    Ok(db.list_all())
}

enum EntryState {
    Date,
    Time,
    TimeAt,
    TimeScheduled,
    TimeScheduledHalf,
    Notify,
    NotifyTime,
    Detail,
}

fn parse_entry(args: Vec<String>) -> Result<Record, anyhow::Error> {
    let mut record = Record::build();
    let mut state = EntryState::Date;

    let mut scheduled_first: Option<time::Time> = None;

    for arg in &args {
        match state {
            EntryState::Date => {
                record.set_date(parse_date(arg.to_string())?);
                state = EntryState::Time;
            }
            EntryState::Time => match arg.as_str() {
                "at" => state = EntryState::TimeAt,
                "from" => state = EntryState::TimeScheduled,
                _ => return Err(anyhow!("Time must be 'from' or 'at'")),
            },
            EntryState::TimeAt => {
                record.set_at(Some(parse_time(arg.to_string())?));
                state = EntryState::Notify;
            }
            EntryState::TimeScheduled => {
                scheduled_first = Some(parse_time(arg.to_string())?);
                state = EntryState::TimeScheduledHalf;
            }
            EntryState::TimeScheduledHalf => match arg.as_str() {
                "to" | "until" => {}
                _ => {
                    record.set_scheduled(Some((
                        scheduled_first.unwrap(),
                        parse_time(arg.to_string())?,
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
                    let duration = fancy_duration::FancyDuration::<time::Duration>::parse(arg)?;
                    if let Some(at) = record.at() {
                        record.add_notification(at - duration.duration());
                    } else if let Some(scheduled) = record.scheduled() {
                        record.add_notification(scheduled.0 - duration.duration());
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

    Ok(record)
}

fn parse_date(s: String) -> Result<time::Date, anyhow::Error> {
    let regex = regex::Regex::new(r#"[/.-]"#)?;
    let split = regex.split(&s);
    let parts = split.collect::<Vec<&str>>();
    match parts.len() {
        3 => {
            // FIXME this should be locale-based
            Ok(time::Date::from_calendar_date(
                parts[0].parse()?,
                time::Month::try_from(parts[1].parse::<u8>()?)?,
                parts[2].parse()?,
            )?)
        }
        2 => {
            // FIXME this should be locale-based
            Ok(time::Date::from_calendar_date(
                time::OffsetDateTime::now_utc().year(),
                time::Month::try_from(parts[0].parse::<u8>()?)?,
                parts[1].parse()?,
            )?)
        }
        1 => {
            // FIXME this should be locale-based
            let now = time::OffsetDateTime::now_utc();
            Ok(time::Date::from_calendar_date(
                now.year(),
                now.month(),
                parts[0].parse()?,
            )?)
        }
        _ => return Err(anyhow!("Cannot parse date")),
    }
}

fn parse_time(s: String) -> Result<time::Time, anyhow::Error> {
    let regex = regex::Regex::new(r#"[:.]"#)?;
    let split = regex.split(&s);
    let parts = split.collect::<Vec<&str>>();

    match parts.len() {
        3 => Ok(time::Time::from_hms(
            parts[0].parse()?,
            parts[1].parse()?,
            parts[2].parse()?,
        )?),
        2 => {
            let regex = regex::Regex::new(r#"(\d+)(\D+)"#)?;
            if let Some(captures) = regex.captures(parts[1]) {
                let hour: u8 = parts[0].parse()?;

                let minute: u8 = if let Some(minute) = captures.get(1) {
                    minute.as_str().parse()?
                } else {
                    return Err(anyhow!("Cannot parse time"));
                };

                if let Some(designation) = captures.get(2) {
                    match designation.as_str() {
                        "pm" | "PM" => Ok(time::Time::from_hms(
                            if hour == 12 { 12 } else { hour + 12 },
                            minute,
                            0,
                        )?),
                        "am" | "AM" => Ok(time::Time::from_hms(
                            if hour == 12 { 0 } else { hour },
                            minute,
                            0,
                        )?),
                        _ => Err(anyhow!("Cannot parse time")),
                    }
                } else {
                    Err(anyhow!("Cannot parse time"))
                }
            } else {
                Ok(time::Time::from_hms(
                    parts[0].parse()?,
                    parts[1].parse()?,
                    0,
                )?)
            }
        }
        1 => {
            let regex = regex::Regex::new(r#"(\d+)(\D*)"#)?;
            if let Some(captures) = regex.captures(parts[0]) {
                let hour: u8 = if let Some(hour) = captures.get(1) {
                    hour.as_str().parse()?
                } else {
                    return Err(anyhow!("Cannot parse time"));
                };

                if let Some(designation) = captures.get(2) {
                    match designation.as_str() {
                        "pm" | "PM" => Ok(time::Time::from_hms(
                            if hour == 12 { 12 } else { hour + 12 },
                            0,
                            0,
                        )?),
                        "am" | "AM" => Ok(time::Time::from_hms(
                            if hour == 12 { 0 } else { hour },
                            0,
                            0,
                        )?),
                        "" => {
                            if time::OffsetDateTime::now_utc().hour() > 12 {
                                Ok(time::Time::from_hms(
                                    if hour == 12 { 12 } else { hour + 12 },
                                    0,
                                    0,
                                )?)
                            } else {
                                Ok(time::Time::from_hms(
                                    if hour == 12 { 0 } else { hour },
                                    0,
                                    0,
                                )?)
                            }
                        }
                        _ => Err(anyhow!("Cannot parse time")),
                    }
                } else {
                    Err(anyhow!("Cannot parse time"))
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

        let table = vec![
            (
                "2018-10-23",
                time::Date::from_calendar_date(2018, time::Month::October, 23).unwrap(),
            ),
            (
                "2018/10/23",
                time::Date::from_calendar_date(2018, time::Month::October, 23).unwrap(),
            ),
            (
                "2018.10.23",
                time::Date::from_calendar_date(2018, time::Month::October, 23).unwrap(),
            ),
            (
                "10.23",
                time::Date::from_calendar_date(
                    time::OffsetDateTime::now_utc().year(),
                    time::Month::October,
                    23,
                )
                .unwrap(),
            ),
            (
                "10/23",
                time::Date::from_calendar_date(
                    time::OffsetDateTime::now_utc().year(),
                    time::Month::October,
                    23,
                )
                .unwrap(),
            ),
            (
                "10-23",
                time::Date::from_calendar_date(
                    time::OffsetDateTime::now_utc().year(),
                    time::Month::October,
                    23,
                )
                .unwrap(),
            ),
            (
                "23",
                time::Date::from_calendar_date(
                    time::OffsetDateTime::now_utc().year(),
                    time::OffsetDateTime::now_utc().month(),
                    23,
                )
                .unwrap(),
            ),
        ];

        for (to_parse, t) in table {
            assert_eq!(parse_date(to_parse.to_string()).unwrap(), t)
        }
    }

    #[test]
    fn test_parse_time() {
        use super::parse_time;

        let pm = time::OffsetDateTime::now_utc().hour() > 12;

        let table = vec![
            ("8:00:00", time::Time::from_hms(8, 0, 0).unwrap()),
            ("8:12:56", time::Time::from_hms(8, 12, 56).unwrap()),
            ("8:00", time::Time::from_hms(8, 0, 0).unwrap()),
            ("8am", time::Time::from_hms(8, 0, 0).unwrap()),
            ("8:00pm", time::Time::from_hms(20, 0, 0).unwrap()),
            ("8pm", time::Time::from_hms(20, 0, 0).unwrap()),
            ("8:30pm", time::Time::from_hms(20, 30, 0).unwrap()),
            (
                "8",
                time::Time::from_hms(if pm { 20 } else { 8 }, 0, 0).unwrap(),
            ),
            (
                "8:30",
                time::Time::from_hms(if pm { 20 } else { 8 }, 30, 0).unwrap(),
            ),
        ];

        for (to_parse, t) in table {
            assert_eq!(parse_time(to_parse.to_string()).unwrap(), t)
        }
    }

    #[test]
    fn test_parse_entry() {
        use super::parse_entry;
        use crate::record::Record;

        let now = time::OffsetDateTime::now_utc();
        let pm = now.hour() > 12;

        let record = Record::build();

        let mut soda = record.clone();
        soda.set_date(time::Date::from_calendar_date(now.year(), time::Month::August, 5).unwrap())
            .set_at(Some(
                time::Time::from_hms(if pm { 20 } else { 8 }, 0, 0).unwrap(),
            ))
            .add_notification(time::Time::from_hms(if pm { 19 } else { 7 }, 55, 0).unwrap())
            .set_detail("Get a Soda".to_string());

        let mut birthday = record.clone();
        birthday
            .set_date(time::Date::from_calendar_date(now.year(), time::Month::October, 23).unwrap())
            .set_at(Some(time::Time::from_hms(7, 30, 0).unwrap()))
            .add_notification(time::Time::from_hms(6, 30, 0).unwrap())
            .set_detail("Tell my daughter 'happy birthday'".to_string());

        let mut new_year = record.clone();
        new_year
            .set_date(time::Date::from_calendar_date(now.year(), time::Month::January, 1).unwrap())
            .set_at(Some(time::Time::from_hms(0, 0, 0).unwrap()))
            .set_detail("Happy new year!".to_string());

        let table = vec![
            (
                "08/05 at 8 notify me 5m Get a Soda"
                    .split(" ")
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>(),
                soda,
            ),
            (
                "10/23 at 7:30am notify 1h Tell my daughter 'happy birthday'"
                    .split(" ")
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>(),
                birthday,
            ),
            (
                "1/1 at 12am Happy new year!"
                    .split(" ")
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>(),
                new_year,
            ),
        ];

        for (to_parse, t) in table {
            assert_eq!(parse_entry(to_parse).unwrap(), t)
        }
    }
}
