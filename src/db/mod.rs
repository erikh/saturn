pub mod file;

use crate::record::{Record, RecurringRecord};

pub trait DBLoader {
    fn load(&self) -> dyn DB;
    fn dump(&self) -> dyn DB;
}

pub trait DB {
    fn delete(&mut self, primary_key: u64);
    fn delete_recurrence(&mut self, primary_key: u64);
    fn record(&mut self, record: Record);
    fn record_recurrence(&mut self, record: RecurringRecord);
    fn list_recurrence(&self) -> Vec<RecurringRecord>;
    fn update_recurrence(&mut self);
    fn next_key(&mut self) -> u64;
    fn next_recurrence_key(&mut self) -> u64;
    fn list_today(&self, include_completed: bool) -> Vec<Record>;
    fn list_all(&self, include_completed: bool) -> Vec<Record>;
    fn events_now(&mut self, last: chrono::Duration, include_completed: bool) -> Vec<Record>;
    fn complete_task(&mut self, primary_key: u64);
}
