use crate::db::DB;
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
pub struct RecurringRecord {
    record: Record,
    recurrence: fancy_duration::FancyDuration<chrono::Duration>,
    recurrence_key: u64,
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
        }
    }

    pub fn record(&self) -> Record {
        self.record.clone()
    }

    pub fn recurrence(&self) -> fancy_duration::FancyDuration<chrono::Duration> {
        self.recurrence.clone()
    }

    pub fn recurrence_key(&self) -> u64 {
        self.recurrence_key
    }

    pub fn set_recurrence_key(&mut self, key: u64) {
        self.recurrence_key = key;
    }

    pub fn record_from(&self, primary_key: u64, from: chrono::NaiveDateTime) -> Record {
        let mut record = self.record.clone();
        record.set_primary_key(primary_key);
        record.set_recurrence_key(Some(self.recurrence_key));
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
    date: chrono::NaiveDate,
    typ: RecordType,
    at: Option<chrono::NaiveTime>,
    scheduled: Option<Schedule>,
    all_day: bool,
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
            date: now.date_naive(),
            typ: RecordType::AllDay,
            at: None,
            scheduled: None,
            all_day: true,
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

    pub fn all_day(&self) -> bool {
        self.all_day
    }

    pub fn at(&self) -> Option<chrono::NaiveTime> {
        self.at
    }

    pub fn scheduled(&self) -> Option<Schedule> {
        self.scheduled
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

    pub fn record(&self, mut db: crate::db::memory::MemoryDB) {
        db.record(self.clone())
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

    pub fn set_all_day(&mut self, all_day: bool) -> &mut Self {
        self.at = None;
        self.scheduled = None;
        self.all_day = all_day;
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
        self.all_day = false;
        self.typ = RecordType::At;
        self
    }

    pub fn set_scheduled(&mut self, schedule: Option<Schedule>) -> &mut Self {
        self.scheduled = schedule;
        self.at = None;
        self.all_day = false;
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
