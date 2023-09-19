mod parser;

use self::parser::parse_entry;
use crate::record::{Record, RecurringRecord};

#[derive(Debug, Clone)]
pub struct EntryParser {
    args: Vec<String>,
    use_24h_time: bool,
}

impl EntryParser {
    pub fn new(args: Vec<String>, use_24h_time: bool) -> Self {
        Self { args, use_24h_time }
    }

    pub fn to_record(&self) -> Result<EntryRecord, anyhow::Error> {
        parse_entry(self.args.clone(), self.use_24h_time)
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
