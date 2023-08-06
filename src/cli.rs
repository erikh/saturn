#![allow(dead_code)]
use crate::{db::DB, record::Record};
use std::{
    env::{args, var, Args},
    path::PathBuf,
};

pub struct CLI {
    args: Args,
    filename: PathBuf,
}

impl CLI {
    pub fn new() -> Self {
        CLI {
            args: args(),
            filename: PathBuf::from(
                var("SATURN_DB").unwrap_or(
                    PathBuf::from(var("HOME").unwrap_or("/".to_string()))
                        .join(".saturn.db")
                        .to_str()
                        .unwrap()
                        .to_string(),
                ),
            ),
        }
    }

    pub fn perform(&self) -> Result<(), anyhow::Error> {
        let mut db = if std::fs::metadata(&self.filename).is_ok() {
            DB::load(self.filename.clone())?
        } else {
            DB::default()
        };

        db.record(self.to_record()?);

        Ok(())
    }

    pub fn to_record(&self) -> Result<Record, anyhow::Error> {
        Ok(Record::random())
    }
}
