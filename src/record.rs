use crate::db::DB;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type Fields = BTreeMap<String, String>;
pub type Schedule = (chrono::NaiveTime, chrono::NaiveTime);
pub type Notifications = Vec<chrono::NaiveTime>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RecordType {
    At,
    Schedule,
    AllDay,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PresentedSchedule {
    start: chrono::NaiveTime,
    stop: chrono::NaiveTime,
}

impl std::fmt::Display for PresentedSchedule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!(
            "{} - {}",
            self.start.format("%H:%M"),
            self.stop.format("%H:%M")
        ))
    }
}

impl From<PresentedSchedule> for Schedule {
    fn from(ps: PresentedSchedule) -> Self {
        (ps.start, ps.stop)
    }
}

impl From<Schedule> for PresentedSchedule {
    fn from(s: Schedule) -> Self {
        Self {
            start: s.0,
            stop: s.1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PresentedRecord {
    pub date: chrono::NaiveDate,
    #[serde(rename = "type")]
    pub typ: RecordType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<chrono::NaiveTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduled: Option<PresentedSchedule>,
    pub detail: String,
    pub fields: Fields,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notifications: Option<Notifications>,
    pub completed: bool,
}

impl From<Record> for PresentedRecord {
    fn from(value: Record) -> Self {
        Self {
            date: value.date,
            typ: value.typ,
            at: value.at,
            scheduled: value.scheduled.map(|x| x.into()),
            detail: value.detail,
            fields: value.fields,
            notifications: value.notifications,
            completed: value.completed,
        }
    }
}

impl PresentedRecord {
    pub fn to_record(
        self,
        primary_key: u64,
        recurrence_key: Option<u64>,
        internal_key: Option<String>,
        internal_recurrence_key: Option<String>,
    ) -> Record {
        Record {
            primary_key,
            recurrence_key,
            internal_key,
            internal_recurrence_key,
            date: self.date,
            typ: self.typ,
            at: self.at,
            scheduled: self.scheduled.map(|x| x.into()),
            detail: self.detail,
            fields: self.fields,
            notifications: self.notifications,
            completed: self.completed,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PresentedRecurringRecord {
    pub record: PresentedRecord,
    pub recurrence: fancy_duration::FancyDuration<chrono::Duration>,
}

impl From<RecurringRecord> for PresentedRecurringRecord {
    fn from(value: RecurringRecord) -> Self {
        Self {
            record: value.record.into(),
            recurrence: value.recurrence,
        }
    }
}
impl PresentedRecurringRecord {
    pub fn to_record(
        self,
        primary_key: u64,
        recurrence_key: u64,
        internal_key: Option<String>,
        internal_recurrence_key: Option<String>,
    ) -> RecurringRecord {
        RecurringRecord {
            internal_key: internal_key.clone(),
            recurrence_key,
            record: self.record.to_record(
                primary_key,
                Some(recurrence_key),
                internal_key,
                internal_recurrence_key,
            ),
            recurrence: self.recurrence,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecurringRecord {
    record: Record,
    recurrence: fancy_duration::FancyDuration<chrono::Duration>,
    recurrence_key: u64,
    internal_key: Option<String>,
}

#[derive(Clone, Debug)]
enum RuleFrequency {
    Daily,
    Monthly,
    Weekly,
    Yearly,
}

impl ToString for RuleFrequency {
    fn to_string(&self) -> String {
        match self {
            RuleFrequency::Daily => "daily",
            RuleFrequency::Monthly => "monthly",
            RuleFrequency::Yearly => "yearly",
            RuleFrequency::Weekly => "weekly",
        }
        .to_uppercase()
    }
}

impl std::str::FromStr for RuleFrequency {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "daily" => Ok(RuleFrequency::Daily),
            "yearly" => Ok(RuleFrequency::Yearly),
            "monthly" => Ok(RuleFrequency::Monthly),
            "weekly" => Ok(RuleFrequency::Weekly),
            _ => Err(anyhow!("Invalid frequency {}", s)),
        }
    }
}

impl RecurringRecord {
    pub fn new(
        record: Record,
        recurrence: fancy_duration::FancyDuration<chrono::Duration>,
    ) -> Self {
        Self {
            record,
            recurrence,
            recurrence_key: 0,
            internal_key: None,
        }
    }

    pub fn from_rrule(record: Record, rrule: String) -> Result<Self> {
        let parts = rrule.split(':').collect::<Vec<&str>>();

        if parts[0] == "RRULE" {
            let tokens = parts[1]
                .split(';')
                .map(|s| s.split('=').collect::<Vec<&str>>());
            let mut freq: Option<RuleFrequency> = None;
            let mut interval: Option<i64> = None;

            for pair in tokens {
                match pair[0] {
                    "FREQ" => {
                        freq = Some(pair[1].parse()?);
                    }
                    "INTERVAL" => {
                        interval = Some(pair[1].parse()?);
                    }
                    _ => {}
                }

                if freq.is_some() && interval.is_some() {
                    break;
                }
            }

            if let Some(freq) = freq {
                if let Some(interval) = interval {
                    return Ok(Self::new(
                        record,
                        fancy_duration::FancyDuration::new(match freq {
                            RuleFrequency::Daily => chrono::Duration::days(interval),
                            RuleFrequency::Yearly => chrono::Duration::weeks(interval) * 52,
                            RuleFrequency::Weekly => chrono::Duration::weeks(interval),
                            RuleFrequency::Monthly => chrono::Duration::days(interval) * 30,
                        }),
                    ));
                }
            }
        }

        Err(anyhow!("Recurring data cannot be parsed"))
    }

    pub fn to_rrule(&self) -> String {
        let recur = self.recurrence.duration();

        let freq = if recur < chrono::Duration::days(30) {
            ("DAILY", recur.num_days())
        } else if recur < chrono::Duration::weeks(52) {
            ("MONTHLY", recur.num_days() / 30)
        } else {
            ("YEARLY", recur.num_weeks() * 52)
        };

        format!("RRULE:FREQ={};INTERVAL={}", freq.0, freq.1)
    }

    pub fn record(&mut self) -> &mut Record {
        &mut self.record
    }

    pub fn recurrence(&self) -> fancy_duration::FancyDuration<chrono::Duration> {
        self.recurrence.clone()
    }

    pub fn recurrence_key(&self) -> u64 {
        self.recurrence_key
    }

    pub fn set_record(&mut self, record: Record) {
        self.record = record;
    }

    pub fn set_recurrence_key(&mut self, key: u64) {
        self.recurrence_key = key;
        self.record().set_recurrence_key(Some(key));
    }

    pub fn internal_key(&self) -> Option<String> {
        self.internal_key.clone()
    }

    pub fn set_internal_key(&mut self, key: Option<String>) {
        self.internal_key = key.clone();
        self.record().set_internal_recurrence_key(key);
    }

    pub fn record_from(&self, primary_key: u64, from: chrono::NaiveDateTime) -> Record {
        let mut record = self.record.clone();
        record.set_primary_key(primary_key);
        record.set_recurrence_key(Some(self.recurrence_key));
        record.set_internal_recurrence_key(self.internal_key.clone());
        record.set_date(from.date());
        match record.record_type() {
            RecordType::At => {
                record.set_at(Some(from.time()));
            }
            RecordType::AllDay => {}
            RecordType::Schedule => {
                let schedule = record.scheduled().unwrap();
                let duration = schedule.1 - schedule.0;
                record.set_scheduled(Some((from.time(), from.time() + duration)));
            }
        };
        record
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Record {
    primary_key: u64,
    recurrence_key: Option<u64>,
    internal_key: Option<String>,
    internal_recurrence_key: Option<String>,
    date: chrono::NaiveDate,
    typ: RecordType,
    at: Option<chrono::NaiveTime>,
    scheduled: Option<Schedule>,
    detail: String,
    fields: Fields,
    notifications: Option<Notifications>,
    completed: bool,
}

impl Default for Record {
    fn default() -> Self {
        let now = chrono::Local::now();
        Self {
            primary_key: 0,
            recurrence_key: None,
            internal_key: None,
            internal_recurrence_key: None,
            date: now.date_naive(),
            typ: RecordType::AllDay,
            at: None,
            scheduled: None,
            detail: String::new(),
            fields: Fields::default(),
            notifications: None,
            completed: false,
        }
    }
}

impl Record {
    pub fn primary_key(&self) -> u64 {
        self.primary_key
    }

    pub fn recurrence_key(&self) -> Option<u64> {
        self.recurrence_key
    }

    pub fn internal_recurrence_key(&self) -> Option<String> {
        self.internal_recurrence_key.clone()
    }

    pub fn internal_key(&self) -> Option<String> {
        self.internal_key.clone()
    }

    pub fn set_internal_key(&mut self, key: Option<String>) {
        self.internal_key = key
    }

    pub fn record_type(&self) -> RecordType {
        self.typ.clone()
    }

    pub fn datetime(&self) -> chrono::DateTime<chrono::Local> {
        let time = match self.record_type() {
            RecordType::At => self.at.unwrap(),
            RecordType::AllDay => chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
            RecordType::Schedule => self.scheduled.unwrap().0,
        };

        chrono::NaiveDateTime::new(self.date, time)
            .and_local_timezone(chrono::Local::now().timezone())
            .unwrap()
    }

    pub fn completed(&self) -> bool {
        self.completed
    }

    pub fn date(&self) -> chrono::NaiveDate {
        self.date
    }

    pub fn at(&self) -> Option<chrono::NaiveTime> {
        self.at
    }

    pub fn scheduled(&self) -> Option<Schedule> {
        self.scheduled
    }

    pub fn all_day(&self) -> bool {
        matches!(self.typ, RecordType::AllDay)
    }

    pub fn detail(&self) -> String {
        self.detail.clone()
    }

    pub fn fields(&self) -> Fields {
        self.fields.clone()
    }

    pub fn notifications(&self) -> Option<Notifications> {
        self.notifications.clone()
    }

    pub fn build() -> Self {
        Self::default()
    }

    pub async fn record(&self, mut db: crate::db::memory::MemoryDB) -> Result<()> {
        db.record(self.clone()).await
    }

    pub fn set_internal_recurrence_key(&mut self, internal_recurrence_key: Option<String>) {
        self.internal_recurrence_key = internal_recurrence_key
    }

    pub fn set_primary_key(&mut self, primary_key: u64) -> &mut Self {
        self.primary_key = primary_key;
        self
    }

    pub fn set_recurrence_key(&mut self, key: Option<u64>) -> &mut Self {
        self.recurrence_key = key;
        self
    }

    pub fn set_record_type(&mut self, typ: RecordType) -> &mut Self {
        self.typ = typ;
        self
    }

    pub fn set_all_day(&mut self) -> &mut Self {
        self.at = None;
        self.scheduled = None;
        self.typ = RecordType::AllDay;
        self
    }

    pub fn set_completed(&mut self, completed: bool) -> &mut Self {
        self.completed = completed;
        self
    }

    pub fn set_date(&mut self, date: chrono::NaiveDate) -> &mut Self {
        self.date = date;
        self
    }

    pub fn set_at(&mut self, at: Option<chrono::NaiveTime>) -> &mut Self {
        self.at = at;
        self.scheduled = None;
        self.typ = RecordType::At;
        self
    }

    pub fn set_scheduled(&mut self, schedule: Option<Schedule>) -> &mut Self {
        self.scheduled = schedule;
        self.at = None;
        self.typ = RecordType::Schedule;
        self
    }

    pub fn set_detail(&mut self, detail: String) -> &mut Self {
        self.detail = detail;
        self
    }

    pub fn add_field(&mut self, field: String, content: String) -> &mut Self {
        self.fields.insert(field, content);
        self
    }

    pub fn add_notification(&mut self, notification: chrono::NaiveTime) -> &mut Self {
        if let Some(notifications) = &mut self.notifications {
            notifications.push(notification)
        } else {
            self.notifications = Some(vec![notification])
        }

        self
    }

    pub fn set_notifications(&mut self, notifications: Option<Vec<chrono::NaiveTime>>) {
        self.notifications = notifications
    }
}

pub fn sort_records(a: &Record, b: &Record) -> std::cmp::Ordering {
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
