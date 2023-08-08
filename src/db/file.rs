use crate::record::{Record, RecurringRecord};
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, os::unix::io::FromRawFd};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UnixFileDB {
    primary_key: u64,
    records: BTreeMap<chrono::NaiveDate, Vec<Record>>,
    recurrence_key: u64,
    recurring: Vec<RecurringRecord>,
}

impl UnixFileDB {
    pub fn load(filename: std::path::PathBuf) -> Result<Self, anyhow::Error> {
        unsafe {
            let fd = nix::libc::open(
                std::ffi::CString::from_vec_unchecked(
                    filename.to_str().unwrap().as_bytes().to_vec(),
                )
                .as_ptr(),
                nix::libc::O_RDONLY,
            );
            if fd < 0 {
                return Err(anyhow!(std::ffi::CStr::from_ptr(nix::libc::strerror(
                    nix::errno::errno()
                ))
                .to_str()
                .unwrap()
                .to_string()));
            }

            if nix::libc::flock(fd, nix::libc::LOCK_EX) != 0 {
                return Err(anyhow!(std::ffi::CStr::from_ptr(nix::libc::strerror(
                    nix::errno::errno()
                ))
                .to_str()
                .unwrap()
                .to_string()));
            }

            Ok(ciborium::from_reader(std::fs::File::from_raw_fd(fd))?)
        }
    }

    pub fn dump(&mut self, filename: std::path::PathBuf) -> Result<(), anyhow::Error> {
        unsafe {
            let fd = nix::libc::open(
                std::ffi::CString::from_vec_unchecked(
                    filename.to_str().unwrap().as_bytes().to_vec(),
                )
                .as_ptr(),
                nix::libc::O_WRONLY | nix::libc::O_TRUNC | nix::libc::O_CREAT,
            );
            if fd < 0 {
                return Err(anyhow!(std::ffi::CStr::from_ptr(nix::libc::strerror(
                    nix::errno::errno()
                ))
                .to_str()
                .unwrap()
                .to_string()));
            }

            if nix::libc::flock(fd, nix::libc::LOCK_EX) != 0 {
                return Err(anyhow!(std::ffi::CStr::from_ptr(nix::libc::strerror(
                    nix::errno::errno()
                ))
                .to_str()
                .unwrap()
                .to_string()));
            }

            if nix::libc::chmod(
                std::ffi::CString::from_vec_unchecked(
                    filename.to_str().unwrap().as_bytes().to_vec(),
                )
                .as_ptr(),
                nix::libc::S_IRUSR | nix::libc::S_IWUSR,
            ) != 0
            {
                return Err(anyhow!(std::ffi::CStr::from_ptr(nix::libc::strerror(
                    nix::errno::errno()
                ))
                .to_str()
                .unwrap()
                .to_string()));
            }

            self.update_recurrence();

            Ok(ciborium::into_writer(self, std::fs::File::from_raw_fd(fd))?)
        }
    }

    pub fn delete(&mut self, primary_key: u64) {
        for (key, list) in self.records.clone() {
            let mut new = Vec::new();
            for record in list {
                if record.primary_key() != primary_key {
                    new.push(record.clone());
                }
            }

            self.records.insert(key, new);
        }
    }

    pub fn delete_recurrence(&mut self, primary_key: u64) {
        let mut new = Vec::new();

        for entry in &self.recurring {
            if entry.recurrence_key() != primary_key {
                new.push(entry.clone());
            }
        }

        self.recurring.clear();
        self.recurring.append(&mut new);
    }

    pub fn record(&mut self, record: Record) {
        if let Some(item) = self.records.get_mut(&record.date()) {
            item.push(record);
        } else {
            self.records.insert(record.date(), vec![record]);
        }
    }

    pub fn record_recurrence(&mut self, record: RecurringRecord) {
        self.recurring.push(record);
    }

    pub fn list_recurrence(&self) -> Vec<RecurringRecord> {
        self.recurring.clone()
    }

    pub fn update_recurrence(&mut self) {
        let now = chrono::Local::now();

        for recur in self.recurring.clone() {
            let mut seen: Option<Record> = None;
            let mut begin = now - recur.recurrence().duration();
            while begin.date_naive() <= now.date_naive() {
                if let Some(items) = self.records.get(&begin.date_naive()) {
                    for item in items {
                        if let Some(key) = item.recurrence_key() {
                            if key == recur.recurrence_key()
                                && item.datetime() - recur.recurrence().duration() < now
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
                    let key = self.next_key();
                    self.record(recur.record_from(key, dt.naive_local()));
                    dt += duration;
                    if dt >= now {
                        break;
                    }
                }
            }
        }
    }

    pub fn next_key(&mut self) -> u64 {
        let key = self.primary_key;
        self.primary_key += 1;
        key
    }

    pub fn next_recurrence_key(&mut self) -> u64 {
        let key = self.recurrence_key;
        self.recurrence_key += 1;
        key
    }

    pub fn list_today(&self, include_completed: bool) -> Vec<Record> {
        self.records
            .get(&chrono::Local::now().date_naive())
            .unwrap_or(&Vec::new())
            .iter()
            .filter_map(|v| {
                if v.completed() && !include_completed {
                    None
                } else {
                    Some(v.clone())
                }
            })
            .collect::<Vec<Record>>()
    }

    pub fn list_all(&self, include_completed: bool) -> Vec<Record> {
        self.records
            .iter()
            .flat_map(|(_, v)| v.clone())
            .filter_map(|v| {
                if v.completed() && !include_completed {
                    None
                } else {
                    Some(v)
                }
            })
            .collect::<Vec<Record>>()
    }

    pub fn events_now(&mut self, last: chrono::Duration, include_completed: bool) -> Vec<Record> {
        let mut ret = Vec::new();
        let now = chrono::Local::now();

        let mut records = self
            .records
            .get_mut(&chrono::Local::now().date_naive())
            .unwrap_or(&mut Vec::new())
            .clone();

        let mut next_day = self
            .records
            .get_mut(&(chrono::Local::now() + chrono::Duration::days(1)).date_naive())
            .unwrap_or(&mut Vec::new())
            .clone();

        records.append(&mut next_day);

        for mut item in records {
            if item.completed() && !include_completed {
                continue;
            }

            if let Some(at) = item.at() {
                if at - now.time() < last && now.time() < at {
                    ret.push(item.clone());
                }
            } else if let Some(schedule) = item.scheduled() {
                if (schedule.0 - last) < now.time() && (schedule.1 + last) > now.time() {
                    ret.push(item.clone())
                }
            } else if item.all_day() {
                if item.date() - chrono::Duration::days(1) == now.date_naive()
                    && now.time() > chrono::NaiveTime::from_hms_opt(23, 59, 0).unwrap() - last
                {
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
                        } else if item.all_day() {
                            if item.date() - chrono::Duration::days(1) == now.date_naive()
                                && now.time()
                                    > chrono::NaiveTime::from_hms_opt(23, 59, 0).unwrap() - last
                            {
                                if !pushed {
                                    ret.push(item.clone());
                                    pushed = true;
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

    pub fn complete_task(&mut self, primary_key: u64) {
        for (_, list) in &mut self.records {
            for record in list {
                if record.primary_key() == primary_key {
                    record.set_completed(true);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_recording() {
        use super::UnixFileDB;
        use crate::record::Record;

        let mut db = UnixFileDB::default();

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

        let res = UnixFileDB::load(f.path().to_path_buf());
        assert!(res.is_ok());

        let db2 = res.unwrap();
        assert_eq!(db.primary_key, db2.primary_key);
        assert_eq!(db.records, db2.records);
    }
}
