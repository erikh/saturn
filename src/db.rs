use crate::record::Record;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DB(BTreeMap<time::Date, Vec<Record>>);

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
        if let Some(item) = self.0.get_mut(&record.date()) {
            item.push(record);
        } else {
            self.0.insert(record.date(), vec![record]);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_recording() {
        use super::DB;
        use crate::record::Record;

        let mut db = DB::default();

        for _ in 0..(rand::random::<usize>() % 50) + 1 {
            db.record(Record::random());
        }

        let f = tempfile::NamedTempFile::new().unwrap();
        assert!(db.dump(f.path().to_path_buf()).is_ok());

        let res = DB::load(f.path().to_path_buf());
        assert!(res.is_ok());

        let db2 = res.unwrap();
        assert_eq!(db.0, db2.0);
    }
}
