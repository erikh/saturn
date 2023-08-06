use crate::record::Record;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DB {
    primary_key: u64,
    records: BTreeMap<chrono::NaiveDate, Vec<Record>>,
}

impl DB {
    pub fn load(filename: std::path::PathBuf) -> Result<Self, anyhow::Error> {
        let mut io = std::fs::OpenOptions::new();
        io.read(true);
        let io = io.open(filename)?;

        Ok(ciborium::from_reader(io)?)
    }

    pub fn dump(&self, filename: std::path::PathBuf) -> Result<(), anyhow::Error> {
        let mut io = std::fs::OpenOptions::new();
        io.truncate(true);
        io.write(true);
        io.create(true);
        let io = io.open(filename)?;

        Ok(ciborium::into_writer(self, io)?)
    }

    pub fn record(&mut self, record: Record) {
        if let Some(item) = self.records.get_mut(&record.date()) {
            item.push(record);
        } else {
            self.records.insert(record.date(), vec![record]);
        }
    }

    pub fn next_key(&mut self) -> u64 {
        let key = self.primary_key;
        self.primary_key += 1;
        key
    }

    pub fn list_today(&self) -> Vec<Record> {
        self.records
            .get(&chrono::Local::now().date_naive())
            .unwrap_or(&Vec::new())
            .clone()
    }

    pub fn list_all(&self) -> Vec<Record> {
        self.records
            .iter()
            .flat_map(|(_, v)| v.clone())
            .collect::<Vec<Record>>()
    }

    pub fn events_now(&mut self, last: chrono::Duration) -> Vec<Record> {
        let mut ret = Vec::new();
        let now = chrono::Local::now();

        for item in self
            .records
            .get_mut(&chrono::Local::now().date_naive())
            .unwrap_or(&mut Vec::new())
        {
            if let Some(at) = item.at() {
                if at - now.time() < last && now.time() < at {
                    ret.push(item.clone());
                }
            } else if let Some(schedule) = item.scheduled() {
                if (schedule.0 - last) < now.time() && (schedule.1 + last) > now.time() {
                    ret.push(item.clone())
                }
            }

            if let Some(notifications) = item.notifications() {
                let mut new = Vec::new();
                let mut pushed = false;

                for notification in notifications {
                    if notification < now.time() {
                        if let Some(at) = item.at() {
                            if now.time() < at {
                                if !pushed {
                                    ret.push(item.clone());
                                    pushed = true
                                }
                            }
                        } else if let Some(schedule) = item.scheduled() {
                            if now.time() < schedule.0 {
                                if !pushed {
                                    ret.push(item.clone());
                                    pushed = true
                                }
                            }
                        }
                    } else {
                        new.push(notification);
                    }
                }

                item.set_notifications(Some(new));
            }
        }

        ret
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_recording() {
        use super::DB;
        use crate::record::Record;

        let mut db = DB::default();

        for x in 0..(rand::random::<u64>() % 50) + 1 {
            db.record(
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
            );
        }

        let f = tempfile::NamedTempFile::new().unwrap();
        assert!(db.dump(f.path().to_path_buf()).is_ok());

        let res = DB::load(f.path().to_path_buf());
        assert!(res.is_ok());

        let db2 = res.unwrap();
        assert_eq!(db.primary_key, db2.primary_key);
        assert_eq!(db.records, db2.records);
    }
}
