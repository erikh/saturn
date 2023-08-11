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

    fn primary_key(&self) -> u64;
    fn set_primary_key(&mut self, primary_key: u64);
    fn recurrence_key(&self) -> u64;
    fn set_recurrence_key(&mut self, primary_key: u64);

    fn delete(&mut self, primary_key: u64) -> Result<(), anyhow::Error>;
    fn delete_recurrence(&mut self, primary_key: u64) -> Result<(), anyhow::Error>;
    fn record(&mut self, record: Record) -> Result<(), anyhow::Error>;
    fn record_recurrence(&mut self, record: RecurringRecord) -> Result<(), anyhow::Error>;
    fn list_recurrence(&self) -> Result<Vec<RecurringRecord>, anyhow::Error>;
    fn update_recurrence(&mut self) -> Result<(), anyhow::Error>;
    fn list_today(&self, include_completed: bool) -> Result<Vec<Record>, anyhow::Error>;
    fn list_all(&self, include_completed: bool) -> Result<Vec<Record>, anyhow::Error>;
    fn events_now(
        &mut self,
        last: chrono::Duration,
        include_completed: bool,
    ) -> Result<Vec<Record>, anyhow::Error>;
    fn complete_task(&mut self, primary_key: u64) -> Result<(), anyhow::Error>;
}

#[async_trait::async_trait]
pub trait RemoteClient: Sync {
    async fn delete(&self, id: String) -> Result<(), anyhow::Error>;
    async fn delete_recurrence(&self, id: String) -> Result<(), anyhow::Error>;
    async fn record(&self, record: Record) -> Result<(), anyhow::Error>;
    async fn record_recurrence(&self, record: RecurringRecord) -> Result<(), anyhow::Error>;
    async fn list_recurrence(&self) -> Result<Vec<RecurringRecord>, anyhow::Error>;
    async fn update_recurrence(&self) -> Result<(), anyhow::Error>;
    async fn list_today(&self, include_completed: bool) -> Result<Vec<Record>, anyhow::Error>;
    async fn list_all(&self, include_completed: bool) -> Result<Vec<Record>, anyhow::Error>;
    async fn events_now(
        &self,
        last: chrono::Duration,
        include_completed: bool,
    ) -> Result<Vec<Record>, anyhow::Error>;
    async fn complete_task(&self, primary_key: u64) -> Result<(), anyhow::Error>;
}
