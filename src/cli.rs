use crate::{
    config::{Config, DBType, CONFIG_FILENAME, DB_FILENAME},
    db::{
        google::{GoogleLoader, CALENDAR_SCOPE},
        memory::MemoryDB,
        unixfile::UnixFileLoader,
        DB,
    },
    oauth::{oauth_listener, ClientParameters, State},
    record::{Record, RecordType, RecurringRecord},
};
use anyhow::anyhow;
use chrono::{Datelike, Duration, Timelike};
use fancy_duration::FancyDuration;
use google_calendar::Client;
use std::{env::var, path::PathBuf};
use tokio::sync::Mutex;

pub fn saturn_config() -> PathBuf {
    PathBuf::from(var("HOME").unwrap_or("/".to_string())).join(CONFIG_FILENAME)
}

pub fn saturn_db() -> PathBuf {
    PathBuf::from(
        var("SATURN_DB").unwrap_or(
            PathBuf::from(var("HOME").unwrap_or("/".to_string()))
                .join(DB_FILENAME)
                .to_str()
                .unwrap()
                .to_string(),
        ),
    )
}

pub struct EntryParser {
    args: Vec<String>,
}

impl EntryParser {
    pub fn new(args: Vec<String>) -> Self {
        Self { args }
    }

    pub async fn entry(&self) -> Result<(), anyhow::Error> {
        let mut record = self.to_record()?;

        do_db(|db| {
            record.record.set_primary_key(db.next_key());

            if let Some(mut recurrence) = record.recurrence {
                let key = db.next_recurrence_key();
                record.record.set_recurrence_key(Some(key));
                recurrence.set_recurrence_key(key);
                db.record_recurrence(recurrence)?;
            }

            db.record(record.record)
        })
        .await
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

pub fn get_config() -> Result<Config, anyhow::Error> {
    Config::load(saturn_config())
}

pub fn set_db_type(db_type: String) -> Result<(), anyhow::Error> {
    let mut config = get_config()?;
    let typ = match db_type.as_str() {
        "google" => DBType::Google,
        "unixfile" => DBType::UnixFile,
        _ => {
            return Err(anyhow!(
                "Invalid db type: valid types are `google` and `unixfile`"
            ))
        }
    };

    config.set_db_type(typ);
    config.save(saturn_config())?;

    Ok(())
}

pub async fn get_access_token() -> Result<(), anyhow::Error> {
    let mut config = get_config()?;

    if !config.has_client() {
        return Err(anyhow!(
            "You need to configure a client first; see `saturn config set-client`"
        ));
    }

    let state = State::new(Mutex::new(ClientParameters {
        client_id: config.client_id().unwrap(),
        client_secret: config.client_secret().unwrap(),
        redirect_url: None,
        access_key: None,
    }));
    let host = oauth_listener(state.clone()).await?;
    let redirect_url = format!("http://{}", host);

    let calendar = Client::new(
        config.client_id().unwrap(),
        config.client_secret().unwrap(),
        redirect_url.clone(),
        "",
        "",
    );

    let url = calendar.user_consent_url(&[CALENDAR_SCOPE.to_string()]);
    println!("Click on this and login: {}", url);

    loop {
        let lock = state.lock().await;
        if lock.access_key.is_some() {
            config.set_access_token(lock.access_key.clone());
            config.set_redirect_url(Some(redirect_url));
            config.save(saturn_config())?;
            println!("Captured. Thanks!");
            return Ok(());
        }

        tokio::time::sleep(std::time::Duration::new(1, 0)).await;
    }
}

pub fn set_client_info(client_id: String, client_secret: String) -> Result<(), anyhow::Error> {
    let mut config = get_config()?;
    config.set_client_info(client_id, client_secret);
    config.save(saturn_config())
}

pub fn set_sync_window(duration: FancyDuration<Duration>) -> Result<(), anyhow::Error> {
    let mut config = get_config()?;
    config.set_sync_duration(Some(duration));
    config.save(saturn_config())
}

async fn do_db<T>(
    f: impl FnOnce(&mut Box<MemoryDB>) -> Result<T, anyhow::Error>,
) -> Result<T, anyhow::Error> {
    let config = Config::load(saturn_config())?;

    match config.db_type() {
        DBType::UnixFile => {
            let filename = saturn_db();

            let mut db = if std::fs::metadata(&filename).is_ok() {
                UnixFileLoader::new(&filename).load().await?
            } else {
                MemoryDB::new()
            };

            let res = f(&mut db);

            UnixFileLoader::new(&filename).dump(&mut db).await?;

            res
        }
        DBType::Google => {
            let loader = GoogleLoader::new(config)?;
            let mut db = loader.load().await?;

            let res = f(&mut db);

            loader.dump(&mut db).await?;

            res
        }
    }
}

pub async fn list_recurrence() -> Result<Vec<RecurringRecord>, anyhow::Error> {
    do_db(|db| Ok(db.list_recurrence())).await
}

pub async fn complete_task(primary_key: u64) -> Result<(), anyhow::Error> {
    do_db(|db| Ok(db.complete_task(primary_key))).await
}

pub async fn delete_event(primary_key: u64, recur: bool) -> Result<(), anyhow::Error> {
    do_db(|db| {
        if recur {
            db.delete_recurrence(primary_key);
        } else {
            db.delete(primary_key);
        }

        Ok(())
    })
    .await
}

pub async fn events_now(
    last: Duration,
    include_completed: bool,
) -> Result<Vec<Record>, anyhow::Error> {
    do_db(|db| {
        let mut events = db.events_now(last, include_completed);
        events.sort_by(sort_events);
        Ok(events)
    })
    .await
}

pub async fn list_entries(
    all: bool,
    include_completed: bool,
) -> Result<Vec<Record>, anyhow::Error> {
    do_db(|db| {
        let mut list = if all {
            db.list_all(include_completed)
        } else {
            db.list_today(include_completed)
        };
        list.sort_by(sort_events);
        Ok(list)
    })
    .await
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
    let mut recurrence: Option<FancyDuration<Duration>> = None;

    for arg in &args {
        match state {
            EntryState::Recur => {
                recurrence = Some(FancyDuration::<Duration>::parse(arg)?);
                state = EntryState::Date;
            }
            EntryState::Date => {
                match arg.to_lowercase().as_str() {
                    "today" => {
                        record.set_date(chrono::Local::now().date_naive());
                        state = EntryState::Time;
                    }
                    "yesterday" => {
                        record.set_date((chrono::Local::now() - Duration::days(1)).date_naive());
                        state = EntryState::Time;
                    }
                    "tomorrow" => {
                        record.set_date((chrono::Local::now() + Duration::days(1)).date_naive());
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
                    let duration = FancyDuration::<Duration>::parse(arg)?;
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
        use chrono::{Datelike, Duration, Timelike};

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
            .set_date((chrono::Local::now() + Duration::days(1)).date_naive())
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
