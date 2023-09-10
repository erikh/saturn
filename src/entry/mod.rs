mod parser;

use self::parser::parse_entry;
use crate::record::{Record, RecurringRecord};

#[derive(Debug, Clone)]
pub struct EntryParser {
    args: Vec<String>,
}

impl EntryParser {
    pub fn new(args: Vec<String>) -> Self {
        Self { args }
    }

    pub fn to_record(&self) -> Result<EntryRecord, anyhow::Error> {
        parse_entry(self.args.clone())
    }
}

pub enum EntryState {
    Recur,
    Date,
    Time,
    TimeAt,
    TimeScheduled,
    TimeScheduledHalf,
    Notify,
    NotifyTime,
    Detail,
}

#[derive(Debug, PartialEq)]
pub struct EntryRecord {
    record: Record,
    recurrence: Option<RecurringRecord>,
}

impl EntryRecord {
    pub fn record(&self) -> Record {
        self.record.clone()
    }

    pub fn recurrence(&self) -> Option<RecurringRecord> {
        self.recurrence.clone()
    }
}
