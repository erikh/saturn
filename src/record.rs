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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Record {
    primary_key: u64,
    date: chrono::NaiveDate,
    typ: RecordType,
    at: Option<chrono::NaiveTime>,
    scheduled: Option<Schedule>,
    all_day: bool,
    detail: String,
    fields: Fields,
    notifications: Option<Notifications>,
}

impl Default for Record {
    fn default() -> Self {
        let now = chrono::Local::now();
        Self {
            primary_key: 0,
            date: now.date_naive(),
            typ: RecordType::AllDay,
            at: None,
            scheduled: None,
            all_day: true,
            detail: String::new(),
            fields: Fields::default(),
            notifications: None,
        }
    }
}

impl Record {
    pub fn primary_key(&self) -> u64 {
        self.primary_key
    }

    pub fn record_type(&self) -> RecordType {
        self.typ.clone()
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

    pub fn record(&self, mut db: crate::db::DB) {
        db.record(self.clone())
    }

    pub fn set_primary_key(&mut self, primary_key: u64) -> &mut Self {
        self.primary_key = primary_key;
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
