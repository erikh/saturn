use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type Fields = BTreeMap<String, String>;
pub type Schedule = (time::Time, time::Time);
pub type Notifications = Vec<time::Time>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Record {
    date: time::Date,
    at: Option<time::Time>,
    scheduled: Option<Schedule>,
    detail: String,
    fields: Fields,
    notifications: Option<Notifications>,
}

impl Default for Record {
    fn default() -> Self {
        let now = time::OffsetDateTime::now_utc();
        Self {
            date: now.date(),
            at: None,
            scheduled: None,
            detail: String::new(),
            fields: Fields::default(),
            notifications: None,
        }
    }
}

impl Record {
    pub fn random() -> Self {
        let now: time::OffsetDateTime = rand::random();
        let mut build = Self::build();
        build.set_date(now.date());

        if rand::random() {
            build.set_at(Some(rand::random()));
        } else {
            build.set_scheduled(Some((rand::random(), rand::random())));
        }

        build.set_detail("random event".to_string());

        for x in 0..(rand::random::<usize>() % 5) {
            build.add_field(format!("field {}", x), "random field".to_string());
        }

        for _ in 0..(rand::random::<usize>() % 5) {
            build.add_notification(rand::random());
        }

        build
    }

    pub fn date(&self) -> time::Date {
        self.date
    }

    pub fn at(&self) -> Option<time::Time> {
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

    pub fn set_date(&mut self, date: time::Date) -> &mut Self {
        self.date = date;
        self
    }

    pub fn set_at(&mut self, at: Option<time::Time>) -> &mut Self {
        self.at = at;
        self.scheduled = None;
        self
    }

    pub fn set_scheduled(&mut self, schedule: Option<Schedule>) -> &mut Self {
        self.scheduled = schedule;
        self.at = None;
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

    pub fn add_notification(&mut self, notification: time::Time) -> &mut Self {
        if let Some(notifications) = &mut self.notifications {
            notifications.push(notification)
        } else {
            self.notifications = Some(vec![notification])
        }

        self
    }
}
