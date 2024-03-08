use super::time::{parse_date, parse_time};
use crate::record::{Record, RecurringRecord};
use anyhow::{anyhow, Result};
use chrono::Duration;
use fancy_duration::FancyDuration;

#[derive(Debug, Clone)]
pub struct EntryParser {
    args: Vec<String>,
    use_24h_time: bool,
}

impl EntryParser {
    pub fn new(args: Vec<String>, use_24h_time: bool) -> Self {
        Self { args, use_24h_time }
    }

    pub fn to_record(&self) -> Result<EntryRecord> {
        parse_entry(self.args.clone(), self.use_24h_time)
    }
}

pub enum EntryState {
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

impl EntryRecord {
    pub fn record(&self) -> Record {
        self.record.clone()
    }

    pub fn recurrence(&self) -> Option<RecurringRecord> {
        self.recurrence.clone()
    }
}

fn parse_entry(args: Vec<String>, use_24h_time: bool) -> Result<EntryRecord> {
    let mut record = Record::build();
    let mut state = EntryState::Date;

    let mut scheduled_first: Option<chrono::NaiveTime> = None;
    let mut recurrence: Option<FancyDuration<Duration>> = None;

    for arg in &args {
        match state {
            EntryState::Recur => {
                recurrence = Some(FancyDuration::<Duration>::parse(arg)?);
                state = EntryState::Date;
            }
            EntryState::Date => {
                match arg.to_lowercase().as_str() {
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
                    record.set_at(Some(parse_time(arg.to_string(), !use_24h_time)?));
                }
                state = EntryState::Notify;
            }
            EntryState::TimeScheduled => {
                scheduled_first = Some(parse_time(arg.to_string(), !use_24h_time)?);
                state = EntryState::TimeScheduledHalf;
            }
            EntryState::TimeScheduledHalf => match arg.as_str() {
                "to" | "until" => {}
                _ => {
                    record.set_scheduled(Some((
                        scheduled_first.unwrap(),
                        parse_time(arg.to_string(), !use_24h_time)?,
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
                    record.add_notification(duration.duration());
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

#[cfg(test)]
mod tests {
    #[test]
    fn test_parse_entry() {
        use super::parse_entry;
        use crate::{record::Record, time::now};
        use chrono::{Datelike, TimeDelta, Timelike};

        let pm = now().hour() >= 12;
        let record = Record::build();

        let mut today = record.clone();
        today
            .set_date(chrono::Local::now().naive_local().date())
            .set_at(Some(
                chrono::NaiveTime::from_hms_opt(if pm { 20 } else { 8 }, 0, 0).unwrap(),
            ))
            .add_notification(chrono::TimeDelta::try_minutes(5).unwrap_or_default())
            .set_detail("Test Today".to_string());

        let mut soda = record.clone();
        soda.set_date(chrono::NaiveDate::from_ymd_opt(now().year(), 8, 5).unwrap())
            .set_at(Some(
                chrono::NaiveTime::from_hms_opt(if pm { 20 } else { 8 }, 0, 0).unwrap(),
            ))
            .add_notification(chrono::TimeDelta::try_minutes(5).unwrap_or_default())
            .set_detail("Get a Soda".to_string());

        let mut relax = record.clone();
        relax
            .set_date((now() + TimeDelta::try_days(1).unwrap_or_default()).date_naive())
            .set_at(Some(chrono::NaiveTime::from_hms_opt(16, 0, 0).unwrap()))
            .set_detail("Relax".to_string());

        let mut birthday = record.clone();
        birthday
            .set_date(chrono::NaiveDate::from_ymd_opt(now().year(), 10, 23).unwrap())
            .set_at(Some(chrono::NaiveTime::from_hms_opt(7, 30, 0).unwrap()))
            .add_notification(chrono::TimeDelta::try_hours(1).unwrap_or_default())
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
