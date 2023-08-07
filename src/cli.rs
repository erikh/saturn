use crate::{
    db::DB,
    record::{Record, RecordType, RecurringRecord},
};
use anyhow::anyhow;
use chrono::{Datelike, Timelike};
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

        let mut record = self.to_record()?;
        record.record.set_primary_key(db.next_key());

        db.record(record.record);

        if let Some(mut recurrence) = record.recurrence {
            recurrence.set_recurrence_key(db.next_recurrence_key());
            db.record_recurrence(recurrence);
        }

        db.dump(self.filename.clone())?;

        Ok(())
    }

    pub fn to_record(&self) -> Result<EntryRecord, anyhow::Error> {
        parse_entry(self.args.clone())
    }
}

fn sort_events(a: &Record, b: &Record) -> std::cmp::Ordering {
    let cmp = a.date().cmp(&b.date());
    if cmp == std::cmp::Ordering::Equal {
        match a.record_type() {
            RecordType::At => {
                if let Some(a_at) = a.at() {
                    if let Some(b_at) = b.at() {
                        a_at.cmp(&b_at)
                    } else if let Some(b_schedule) = b.scheduled() {
                        a_at.cmp(&b_schedule.0)
                    } else {
                        std::cmp::Ordering::Equal
                    }
                } else {
                    std::cmp::Ordering::Equal
                }
            }
            RecordType::AllDay => {
                if b.record_type() == RecordType::AllDay {
                    a.primary_key().cmp(&b.primary_key())
                } else {
                    std::cmp::Ordering::Less
                }
            }
            RecordType::Schedule => {
                if let Some(a_schedule) = a.scheduled() {
                    if let Some(b_schedule) = b.scheduled() {
                        a_schedule.0.cmp(&b_schedule.0)
                    } else if let Some(b_at) = b.at() {
                        a_schedule.0.cmp(&b_at)
                    } else {
                        std::cmp::Ordering::Equal
                    }
                } else {
                    std::cmp::Ordering::Equal
                }
            }
        }
    } else {
        cmp
    }
}

pub fn list_recurrence() -> Result<Vec<RecurringRecord>, anyhow::Error> {
    let filename = saturn_db();

    let db = if std::fs::metadata(&filename).is_ok() {
        DB::load(filename.clone())?
    } else {
        DB::default()
    };

    Ok(db.list_recurrence())
}

pub fn complete_task(primary_key: u64) -> Result<(), anyhow::Error> {
    let filename = saturn_db();

    let mut db = if std::fs::metadata(&filename).is_ok() {
        DB::load(filename.clone())?
    } else {
        DB::default()
    };

    db.complete_task(primary_key);
    db.dump(filename.clone())
}

pub fn delete_event(primary_key: u64, recur: bool) -> Result<(), anyhow::Error> {
    let filename = saturn_db();

    let mut db = if std::fs::metadata(&filename).is_ok() {
        DB::load(filename.clone())?
    } else {
        DB::default()
    };

    if recur {
        db.delete_recurrence(primary_key);
    } else {
        db.delete(primary_key);
    }

    db.dump(filename.clone())
}

pub fn events_now(
    last: chrono::Duration,
    include_completed: bool,
) -> Result<Vec<Record>, anyhow::Error> {
    let filename = saturn_db();

    let mut db = if std::fs::metadata(&filename).is_ok() {
        DB::load(filename.clone())?
    } else {
        DB::default()
    };

    let mut events = db.events_now(last, include_completed);
    events.sort_by(|a, b| sort_events(a, b));

    db.dump(filename.clone())?;

    Ok(events)
}

pub fn list_entries(all: bool, include_completed: bool) -> Result<Vec<Record>, anyhow::Error> {
    let filename = saturn_db();

    let db = if std::fs::metadata(&filename).is_ok() {
        DB::load(filename.clone())?
    } else {
        DB::default()
    };

    let mut list = if all {
        db.list_all(include_completed)
    } else {
        db.list_today(include_completed)
    };
    list.sort_by(|a, b| sort_events(a, b));

    Ok(list)
}

enum EntryState {
    Recur,
    Date,
    Time,
    TimeAt,
    TimeScheduled,
    TimeScheduledHalf,
    Notify,
    NotifyTime,
    Detail,
}

#[derive(Debug, PartialEq)]
pub struct EntryRecord {
    record: Record,
    recurrence: Option<RecurringRecord>,
}

fn parse_entry(args: Vec<String>) -> Result<EntryRecord, anyhow::Error> {
    let mut record = Record::build();
    let mut state = EntryState::Date;

    let mut scheduled_first: Option<chrono::NaiveTime> = None;
    let mut recurrence: Option<fancy_duration::FancyDuration<chrono::Duration>> = None;

    for arg in &args {
        match state {
            EntryState::Recur => {
                recurrence = Some(fancy_duration::FancyDuration::<chrono::Duration>::parse(
                    arg,
                )?);
                state = EntryState::Date;
            }
            EntryState::Date => {
                match arg.to_lowercase().as_str() {
                    "today" => {
                        record.set_date(chrono::Local::now().date_naive());
                        state = EntryState::Time;
                    }
                    "yesterday" => {
                        record.set_date(
                            (chrono::Local::now() - chrono::Duration::days(1)).date_naive(),
                        );
                        state = EntryState::Time;
                    }
                    "tomorrow" => {
                        record.set_date(
                            (chrono::Local::now() + chrono::Duration::days(1)).date_naive(),
                        );
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
                    record.set_all_day(true);
                    state = EntryState::TimeAt
                }
                "at" => state = EntryState::TimeAt,
                "from" => state = EntryState::TimeScheduled,
                _ => return Err(anyhow!("Time must be 'from' or 'at'")),
            },
            EntryState::TimeAt => {
                if arg != "day" {
                    record.set_at(Some(parse_time(arg.to_string())?));
                }
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
                    let duration = fancy_duration::FancyDuration::<chrono::Duration>::parse(arg)?;
                    if let Some(at) = record.at() {
                        record.add_notification(at - duration.duration());
                    } else if let Some(scheduled) = record.scheduled() {
                        record.add_notification(scheduled.0 - duration.duration());
                    } else if record.all_day() {
                        record.add_notification(
                            chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap() - duration.duration(),
                        );
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

    if let Some(recurrence) = recurrence {
        let rr = RecurringRecord::new(record.clone(), recurrence);

        Ok(EntryRecord {
            record,
            recurrence: Some(rr),
        })
    } else {
        Ok(EntryRecord {
            record,
            recurrence: None,
        })
    }
}

fn parse_date(s: String) -> Result<chrono::NaiveDate, anyhow::Error> {
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
            Ok(chrono::NaiveDate::from_ymd_opt(
                chrono::Local::now().year(),
                parts[0].parse()?,
                parts[1].parse()?,
            )
            .expect("Invalid Date"))
        }
        1 => {
            // FIXME this should be locale-based
            let now = chrono::Local::now();
            Ok(
                chrono::NaiveDate::from_ymd_opt(now.year(), now.month(), parts[0].parse()?)
                    .expect("Invalid Date"),
            )
        }
        _ => return Err(anyhow!("Cannot parse date")),
    }
}

fn parse_time(s: String) -> Result<chrono::NaiveTime, anyhow::Error> {
    let regex = regex::Regex::new(r#"[:.]"#)?;
    let split = regex.split(&s);
    let parts = split.collect::<Vec<&str>>();

    match parts.len() {
        3 => Ok(chrono::NaiveTime::from_hms_opt(
            parts[0].parse()?,
            parts[1].parse()?,
            parts[2].parse()?,
        )
        .expect("Invalid Time")),
        2 => {
            let regex = regex::Regex::new(r#"(\d+)(\D+)"#)?;
            if let Some(captures) = regex.captures(parts[1]) {
                let hour: u32 = parts[0].parse()?;

                let minute: u32 = if let Some(minute) = captures.get(1) {
                    minute.as_str().parse()?
                } else {
                    return Err(anyhow!("Cannot parse time"));
                };

                if let Some(designation) = captures.get(2) {
                    match designation.as_str() {
                        "pm" | "PM" => Ok(chrono::NaiveTime::from_hms_opt(
                            if hour == 12 { 12 } else { hour + 12 },
                            minute,
                            0,
                        )
                        .expect("Invalid Time")),
                        "am" | "AM" => Ok(chrono::NaiveTime::from_hms_opt(
                            if hour == 12 { 0 } else { hour },
                            minute,
                            0,
                        )
                        .expect("Invalid Time")),
                        _ => Err(anyhow!("Cannot parse time")),
                    }
                } else {
                    Err(anyhow!("Cannot parse time"))
                }
            } else {
                let hour = parts[0].parse()?;
                let minute = parts[1].parse()?;

                if chrono::Local::now().hour() >= 12 {
                    Ok(chrono::NaiveTime::from_hms_opt(
                        if hour == 12 { 12 } else { hour + 12 },
                        minute,
                        0,
                    )
                    .expect("Invalid Time"))
                } else {
                    Ok(chrono::NaiveTime::from_hms_opt(
                        if hour == 12 { 0 } else { hour },
                        minute,
                        0,
                    )
                    .expect("Invalid Time"))
                }
            }
        }
        1 => {
            let regex = regex::Regex::new(r#"(\d+)(\D*)"#)?;
            if let Some(captures) = regex.captures(parts[0]) {
                let hour: u32 = if let Some(hour) = captures.get(1) {
                    hour.as_str().parse()?
                } else {
                    return Err(anyhow!("Cannot parse time"));
                };

                if let Some(designation) = captures.get(2) {
                    match designation.as_str() {
                        "pm" | "PM" => Ok(chrono::NaiveTime::from_hms_opt(
                            if hour == 12 { 12 } else { hour + 12 },
                            0,
                            0,
                        )
                        .expect("Invalid Time")),
                        "am" | "AM" => Ok(chrono::NaiveTime::from_hms_opt(
                            if hour == 12 { 0 } else { hour },
                            0,
                            0,
                        )
                        .expect("Invalid Time")),
                        "" => {
                            if chrono::Local::now().hour() >= 12 {
                                Ok(chrono::NaiveTime::from_hms_opt(
                                    if hour == 12 { 12 } else { hour + 12 },
                                    0,
                                    0,
                                )
                                .expect("Invalid Time"))
                            } else {
                                Ok(chrono::NaiveTime::from_hms_opt(
                                    if hour == 12 { 0 } else { hour },
                                    0,
                                    0,
                                )
                                .expect("Invalid Time"))
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
                chrono::NaiveDate::from_ymd_opt(chrono::Local::now().year(), 10, 23).unwrap(),
            ),
            (
                "10/23",
                chrono::NaiveDate::from_ymd_opt(chrono::Local::now().year(), 10, 23).unwrap(),
            ),
            (
                "10-23",
                chrono::NaiveDate::from_ymd_opt(chrono::Local::now().year(), 10, 23).unwrap(),
            ),
            (
                "23",
                chrono::NaiveDate::from_ymd_opt(
                    chrono::Local::now().year(),
                    chrono::Local::now().month(),
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
        use chrono::Timelike;

        let pm = chrono::Local::now().hour() >= 12;

        let table = vec![
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

        for (to_parse, t) in table {
            assert_eq!(parse_time(to_parse.to_string()).unwrap(), t, "{}", to_parse)
        }
    }

    #[test]
    fn test_parse_entry() {
        use super::parse_entry;
        use crate::record::Record;
        use chrono::{Datelike, Timelike};

        let now = chrono::Local::now();
        let pm = now.hour() >= 12;

        let record = Record::build();

        let mut soda = record.clone();
        soda.set_date(chrono::NaiveDate::from_ymd_opt(now.year(), 8, 5).unwrap())
            .set_at(Some(
                chrono::NaiveTime::from_hms_opt(if pm { 20 } else { 8 }, 0, 0).unwrap(),
            ))
            .add_notification(
                chrono::NaiveTime::from_hms_opt(if pm { 19 } else { 7 }, 55, 0).unwrap(),
            )
            .set_detail("Get a Soda".to_string());

        let mut relax = record.clone();
        relax
            .set_date((chrono::Local::now() + chrono::Duration::days(1)).date_naive())
            .set_at(Some(chrono::NaiveTime::from_hms_opt(16, 0, 0).unwrap()))
            .set_detail("Relax".to_string());

        let mut birthday = record.clone();
        birthday
            .set_date(chrono::NaiveDate::from_ymd_opt(now.year(), 10, 23).unwrap())
            .set_at(Some(chrono::NaiveTime::from_hms_opt(7, 30, 0).unwrap()))
            .add_notification(chrono::NaiveTime::from_hms_opt(6, 30, 0).unwrap())
            .set_detail("Tell my daughter 'happy birthday'".to_string());

        let mut new_year = record.clone();
        new_year
            .set_date(chrono::NaiveDate::from_ymd_opt(now.year(), 1, 1).unwrap())
            .set_at(Some(chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap()))
            .set_detail("Happy new year!".to_string());

        let mut christmas = record.clone();
        christmas
            .set_date(chrono::NaiveDate::from_ymd_opt(now.year(), 12, 25).unwrap())
            .set_scheduled(Some((
                chrono::NaiveTime::from_hms_opt(7, 0, 0).unwrap(),
                chrono::NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
            )))
            .set_detail("Christmas Morning".to_string());

        let table = vec![
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
                        .collect::<Vec<String>>()
                )
                .unwrap()
                .record,
                t,
            )
        }
    }
}
