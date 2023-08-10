#![allow(dead_code)]
use crate::record::{Record, RecurringRecord};
use std::collections::BTreeMap;

pub struct RemoteDB {
    primary_key: u64,
    recurring_key: u64,
    id_map: BTreeMap<String, u64>,
    recurring_id_map: BTreeMap<String, u64>,
}

impl RemoteDB {
    fn delete(&mut self, _primary_key: u64) {}

    fn delete_recurrence(&mut self, _primary_key: u64) {}

    fn record(&mut self, _record: Record) {}

    fn record_recurrence(&mut self, _record: RecurringRecord) {}

    fn list_recurrence(&self) -> Vec<RecurringRecord> {
        Vec::new()
    }

    fn update_recurrence(&mut self) {}

    fn next_key(&mut self) -> u64 {
        0
    }

    fn next_recurrence_key(&mut self) -> u64 {
        0
    }

    fn list_today(&self, _include_completed: bool) -> Vec<Record> {
        Vec::new()
    }

    fn list_all(&self, _include_completed: bool) -> Vec<Record> {
        Vec::new()
    }

    fn events_now(&mut self, _last: chrono::Duration, _include_completed: bool) -> Vec<Record> {
        Vec::new()
    }

    fn complete_task(&mut self, _primary_key: u64) {}
}
