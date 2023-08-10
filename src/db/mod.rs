pub mod google;
pub mod memory;
pub mod remote;
pub mod unixfile;

use crate::record::{Record, RecurringRecord};

#[async_trait::async_trait]
pub trait DBLoader<T>
where
    T: DB + serde::Serialize + for<'a> serde::Deserialize<'a>,
{
    async fn load(&self) -> Result<Box<T>, anyhow::Error>;
    async fn dump(&self, db: &mut Box<T>) -> Result<(), anyhow::Error>;
}

pub trait DB {
    fn next_key(&mut self) -> u64 {
        let key = self.primary_key() + 1;
        self.set_primary_key(key);
        key
    }

    fn next_recurrence_key(&mut self) -> u64 {
        let key = self.recurrence_key() + 1;
        self.set_recurrence_key(key);
        key
    }

    fn delete(&mut self, primary_key: u64);
    fn delete_recurrence(&mut self, primary_key: u64);
    fn record(&mut self, record: Record);
    fn record_recurrence(&mut self, record: RecurringRecord);
    fn list_recurrence(&self) -> Vec<RecurringRecord>;
    fn update_recurrence(&mut self);
    fn primary_key(&self) -> u64;
    fn set_primary_key(&mut self, primary_key: u64);
    fn recurrence_key(&self) -> u64;
    fn set_recurrence_key(&mut self, primary_key: u64);
    fn list_today(&self, include_completed: bool) -> Vec<Record>;
    fn list_all(&self, include_completed: bool) -> Vec<Record>;
    fn events_now(&mut self, last: chrono::Duration, include_completed: bool) -> Vec<Record>;
    fn complete_task(&mut self, primary_key: u64);
}
