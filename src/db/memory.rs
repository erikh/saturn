use crate::{
    db::DB,
    filenames::saturn_db,
    record::{Record, RecurringRecord},
    time::now,
};
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::unixfile::UnixFileLoader;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryDB {
    primary_key: u64,
    records: BTreeMap<chrono::NaiveDate, Vec<Record>>,
    recurrence_key: u64,
    recurring: Vec<RecurringRecord>,
}

impl MemoryDB {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl DB for MemoryDB {
    async fn load(&mut self) -> Result<()> {
        let db: Self = UnixFileLoader::new(&saturn_db()).load().await;
        self.primary_key = db.primary_key;
        self.records = db.records;
        self.recurrence_key = db.recurrence_key;
        self.recurring = db.recurring;
        Ok(())
    }

    async fn dump(&self) -> Result<()> {
        UnixFileLoader::new(&saturn_db()).dump(self.clone()).await
    }

    fn primary_key(&self) -> u64 {
        self.primary_key
    }

    fn recurrence_key(&self) -> u64 {
        self.recurrence_key
    }

    fn set_primary_key(&mut self, primary_key: u64) {
        self.primary_key = primary_key;
    }

    fn set_recurrence_key(&mut self, primary_key: u64) {
        self.recurrence_key = primary_key;
    }

    async fn delete(&mut self, primary_key: u64) -> Result<()> {
        for (key, list) in self.records.clone() {
            let mut new = Vec::new();
            for record in list {
                if record.primary_key() != primary_key {
                    new.push(record.clone());
                }
            }

            self.records.insert(key, new);
        }
        Ok(())
    }

    async fn delete_recurrence(&mut self, primary_key: u64) -> Result<Vec<String>> {
        let mut new = Vec::new();

        for entry in &self.recurring {
            if entry.recurrence_key() != primary_key {
                new.push(entry.clone());
            }
        }

        self.recurring.clear();
        self.recurring.append(&mut new);
        Ok(Vec::new())
    }

    async fn record(&mut self, record: Record) -> Result<()> {
        if let Some(item) = self.records.get_mut(&record.date()) {
            item.push(record);
        } else {
            self.records.insert(record.date(), vec![record]);
        }

        Ok(())
    }

    async fn record_recurrence(&mut self, record: RecurringRecord) -> Result<()> {
        self.recurring.push(record);
        Ok(())
    }

    async fn insert_record(&mut self, record: Record) -> Result<()> {
        self.record(record).await
    }

    async fn insert_recurrence(&mut self, record: RecurringRecord) -> Result<()> {
        self.record_recurrence(record).await
    }

    async fn list_recurrence(&mut self) -> Result<Vec<RecurringRecord>> {
        Ok(self.recurring.clone())
    }

    async fn update_recurrence(&mut self) -> Result<()> {
        for recur in self.recurring.clone() {
            let mut seen: Option<Record> = None;
            let mut begin = now() - recur.recurrence().duration() - chrono::Duration::days(7);
            while begin.date_naive() <= now().date_naive() {
                if let Some(items) = self.records.get(&begin.date_naive()) {
                    for item in items {
                        if let Some(key) = item.recurrence_key() {
                            if key == recur.recurrence_key()
                                && item.datetime() - recur.recurrence().duration() < now()
                            {
                                seen = Some(item.clone());
                            }
                        }
                    }
                }
                begin += chrono::Duration::days(1);
            }

            if let Some(seen) = seen {
                let mut dt = seen.datetime();
                let duration = recur.recurrence().duration();

                loop {
                    dt += duration;
                    if dt >= now() {
                        break;
                    }
                    let key = self.next_key();
                    self.record(recur.record_from(key, dt.naive_local()))
                        .await?;
                }
            }
        }

        Ok(())
    }

    async fn list_today(&mut self, include_completed: bool) -> Result<Vec<Record>> {
        Ok(self
            .records
            .get(&now().date_naive())
            .unwrap_or(&Vec::new())
            .iter()
            .filter_map(|v| {
                if v.completed() && !include_completed {
                    None
                } else {
                    Some(v.clone())
                }
            })
            .collect::<Vec<Record>>())
    }

    async fn list_all(&mut self, include_completed: bool) -> Result<Vec<Record>> {
        Ok(self
            .records
            .iter()
            .flat_map(|(_, v)| v.clone())
            .filter_map(|v| {
                if v.completed() && !include_completed {
                    None
                } else {
                    Some(v)
                }
            })
            .collect::<Vec<Record>>())
    }

    async fn events_now(
        &mut self,
        last: chrono::Duration,
        include_completed: bool,
    ) -> Result<Vec<Record>> {
        let mut ret = Vec::new();

        let mut records = self
            .records
            .get_mut(&now().date_naive())
            .unwrap_or(&mut Vec::new())
            .clone();

        let mut next_day = self
            .records
            .get_mut(&(now() + chrono::Duration::days(1)).date_naive())
            .unwrap_or(&mut Vec::new())
            .clone();

        records.append(&mut next_day);

        for mut item in records {
            if item.completed() && !include_completed {
                continue;
            }

            if let Some(at) = item.at() {
                if at - now().time() < last && now().time() < at {
                    ret.push(item.clone());
                }
            } else if let Some(schedule) = item.scheduled() {
                if (schedule.0 - last) < now().time() && (schedule.1 + last) > now().time() {
                    ret.push(item.clone())
                }
            } else if item.all_day()
                && item.date() - chrono::Duration::days(1) == now().date_naive()
                && now().time() > chrono::NaiveTime::from_hms_opt(23, 59, 0).unwrap() - last
            {
                ret.push(item.clone())
            }

            if let Some(notifications) = item.notifications() {
                let mut new = Vec::new();
                let mut pushed = false;

                for notification in notifications {
                    if notification < now().time() {
                        if let Some(at) = item.at() {
                            if now().time() < at && !pushed {
                                ret.push(item.clone());
                                pushed = true
                            }
                        } else if let Some(schedule) = item.scheduled() {
                            if now().time() < schedule.0 && !pushed {
                                ret.push(item.clone());
                                pushed = true
                            }
                        } else if item.all_day()
                            && item.date() - chrono::Duration::days(1) == now().date_naive()
                            && now().time()
                                > chrono::NaiveTime::from_hms_opt(23, 59, 0).unwrap() - last
                            && !pushed
                        {
                            ret.push(item.clone());
                            pushed = true;
                        }
                    } else {
                        new.push(notification);
                    }
                }

                item.set_notifications(Some(new));
            }
        }

        Ok(ret)
    }

    async fn complete_task(&mut self, primary_key: u64) -> Result<()> {
        for list in self.records.values_mut() {
            for record in list {
                if record.primary_key() == primary_key {
                    record.set_completed(true);
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_recording() {
        use crate::db::{memory::MemoryDB, unixfile::UnixFileLoader, DB};
        use crate::record::Record;

        let mut db = MemoryDB::new();

        for x in 0..(rand::random::<u64>() % 50) + 1 {
            assert!(db
                .record(
                    Record::build()
                        .set_primary_key(x)
                        .set_date(
                            chrono::NaiveDate::from_ymd_opt(
                                rand::random::<i32>() % 5 + 2023,
                                rand::random::<u32>() % 12 + 1,
                                rand::random::<u32>() % 28 + 1,
                            )
                            .unwrap(),
                        )
                        .set_at(Some(
                            chrono::NaiveTime::from_hms_opt(
                                rand::random::<u32>() % 24,
                                rand::random::<u32>() % 60,
                                0,
                            )
                            .unwrap(),
                        ))
                        .clone(),
                )
                .await
                .is_ok());
        }

        let f = tempfile::NamedTempFile::new().unwrap();
        assert!(UnixFileLoader::new(&f.path().to_path_buf())
            .dump(db.clone())
            .await
            .is_ok());

        let db2: MemoryDB = UnixFileLoader::new(&f.path().to_path_buf()).load().await;
        assert_eq!(db.primary_key, db2.primary_key);
        assert_eq!(db.records, db2.records);
    }
}
