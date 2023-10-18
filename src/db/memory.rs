use super::unixfile::UnixFileLoader;
use crate::{
    db::DB,
    filenames::saturn_db,
    record::{Record, RecurringRecord},
    time::now,
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::Timelike;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryDB {
    primary_key: u64,
    records: BTreeMap<u64, Record>,
    recurrence_key: u64,
    recurring: BTreeMap<u64, RecurringRecord>,
}

impl MemoryDB {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl DB for MemoryDB {
    async fn load(&mut self) -> Result<()> {
        let db: Self = UnixFileLoader::new(&saturn_db()).load().await?;
        self.primary_key = db.primary_key;
        self.records = db.records;
        self.recurrence_key = db.recurrence_key;
        self.recurring = db.recurring;
        Ok(())
    }

    fn last_updated(&self) -> chrono::DateTime<chrono::Local> {
        now()
    }

    fn set_last_updated(&mut self, _time: chrono::DateTime<chrono::Local>) {}

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
        self.records.remove(&primary_key);
        Ok(())
    }

    async fn delete_recurrence(&mut self, recurrence_key: u64) -> Result<Vec<String>> {
        self.recurring.remove(&recurrence_key);
        Ok(Vec::new()) // FIXME NFI why this is being returned
    }

    async fn record(&mut self, record: Record) -> Result<()> {
        self.records.insert(record.primary_key(), record);
        Ok(())
    }

    async fn record_recurrence(&mut self, record: RecurringRecord) -> Result<()> {
        self.recurring.insert(record.recurrence_key(), record);
        Ok(())
    }

    async fn insert_record(&mut self, record: Record) -> Result<()> {
        self.record(record).await
    }

    async fn insert_recurrence(&mut self, record: RecurringRecord) -> Result<()> {
        self.record_recurrence(record).await
    }

    async fn list_recurrence(&mut self) -> Result<Vec<RecurringRecord>> {
        let mut v = Vec::new();

        for (_, val) in &self.recurring {
            v.push(val.clone());
        }

        Ok(v)
    }

    async fn update_recurrence(&mut self) -> Result<()> {
        let mut recurring = self.recurring.clone();
        let records = self.records.clone();

        for (_, recur) in &mut recurring {
            let mut seen: Option<&Record> = None;

            let mut begin = recur.record().datetime();
            let tomorrow = (now() + chrono::Duration::days(1)).date_naive();

            while begin.date_naive() <= tomorrow {
                for (_, record) in &records {
                    if let Some(key) = record.recurrence_key() {
                        if key == recur.recurrence_key() && record.datetime() == begin {
                            seen = Some(record);
                        }
                    }
                }

                if seen.is_none() {
                    let key = self.next_key();
                    self.record(recur.record_from(key, begin.naive_local()))
                        .await?;
                }

                begin += recur.recurrence().duration();
            }
        }

        Ok(())
    }

    async fn list_today(&mut self, include_completed: bool) -> Result<Vec<Record>> {
        let today = now().date_naive();

        Ok(self
            .records
            .iter()
            .filter_map(|(_, v)| {
                if v.date() != today || (v.completed() && !include_completed) {
                    None
                } else {
                    Some(v.clone())
                }
            })
            .collect::<Vec<Record>>())
    }

    async fn list_all(&mut self, include_completed: bool) -> Result<Vec<Record>> {
        let values = self
            .records
            .iter()
            .filter(|(_, v)| {
                if v.completed() && !include_completed {
                    false
                } else {
                    true
                }
            })
            .collect::<BTreeMap<&u64, &Record>>();

        let mut v = Vec::new();

        for (_, val) in values {
            v.push(val.clone())
        }

        Ok(v)
    }

    async fn events_now(
        &mut self,
        last: chrono::Duration,
        include_completed: bool,
    ) -> Result<Vec<Record>> {
        let mut ret = Vec::new();
        let n = now().date_naive();

        let mut records = Vec::new();

        for record in self.records.iter().filter(|(_, v)| v.date() == n) {
            records.push(record);
        }

        let n = n + chrono::Duration::days(1);

        let mut next_day = self.records.iter().filter(|(_, v)| v.date() == n).collect();

        records.append(&mut next_day);

        for (_, item) in records {
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
            } else {
                let dt = item.datetime();
                let n = now();
                if dt > n && n > dt - last {
                    ret.push(item.clone());
                } else if let Some(notifications) = item.notifications() {
                    for notification in notifications {
                        let dt_window = dt - notification.duration();
                        let dt_time = dt_window
                            .time()
                            .with_second(0)
                            .unwrap()
                            .with_nanosecond(0)
                            .unwrap();
                        let n_time = n.time().with_second(0).unwrap().with_nanosecond(0).unwrap();

                        if dt > n && dt_window.date_naive() == n.date_naive() && dt_time == n_time {
                            ret.push(item.clone());
                            break;
                        }
                    }
                }
            }
        }

        Ok(ret)
    }

    async fn complete_task(&mut self, primary_key: u64) -> Result<()> {
        for record in self.records.values_mut() {
            if record.primary_key() == primary_key {
                record.set_completed(true);
            }
        }

        Ok(())
    }

    async fn get(&mut self, primary_key: u64) -> Result<Record> {
        let mut record: Option<Record> = None;
        for r in self.records.values() {
            if primary_key == r.primary_key() {
                record = Some(r.clone());
                break;
            }
        }

        record.ok_or(anyhow!("No Record Found"))
    }

    async fn get_recurring(&mut self, recurrence_key: u64) -> Result<RecurringRecord> {
        self.recurring
            .get(&recurrence_key)
            .ok_or(anyhow!("No Record Found"))
            .cloned()
    }

    async fn update(&mut self, record: Record) -> Result<()> {
        self.records.insert(record.primary_key(), record);
        Ok(())
    }

    async fn update_recurring(&mut self, record: RecurringRecord) -> Result<()> {
        self.recurring.insert(record.recurrence_key(), record);
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

        let db2: MemoryDB = UnixFileLoader::new(&f.path().to_path_buf())
            .load()
            .await
            .unwrap();
        assert_eq!(db.primary_key, db2.primary_key);
        assert_eq!(db.records, db2.records);
    }
}
