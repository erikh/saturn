pub mod google;
pub mod google_macros;
pub mod memory;
pub mod remote;
pub mod unixfile;

use crate::{
    entry::EntryParser,
    record::{Record, RecurringRecord},
};
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait DB: Send {
    async fn load(&mut self) -> Result<()>;
    async fn dump(&self) -> Result<()>;

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

    async fn record_entry(&mut self, entry: EntryParser) -> Result<()> {
        let record = entry.to_record()?;
        let recurrence = record.recurrence();
        let mut record = record.record();
        record.set_primary_key(self.next_key());

        if let Some(mut recurrence) = recurrence {
            let key = if let Some(key) = record.recurrence_key() {
                key
            } else {
                self.next_recurrence_key()
            };
            record.set_recurrence_key(Some(key));
            recurrence.set_record(record);
            recurrence.set_recurrence_key(key);
            self.record_recurrence(recurrence).await?;
        } else {
            self.record(record).await?;
        }

        Ok(())
    }

    async fn update(&mut self, record: Record) -> Result<()>;
    async fn update_recurring(&mut self, record: RecurringRecord) -> Result<()>;
    async fn get(&mut self, primary_key: u64) -> Result<Record>;
    async fn get_recurring(&mut self, primary_key: u64) -> Result<RecurringRecord>;
    async fn delete(&mut self, primary_key: u64) -> Result<()>;
    async fn delete_recurrence(&mut self, primary_key: u64) -> Result<Vec<String>>;
    async fn record(&mut self, record: Record) -> Result<()>;
    async fn record_recurrence(&mut self, record: RecurringRecord) -> Result<()>;
    async fn insert_record(&mut self, record: Record) -> Result<()>;
    async fn insert_recurrence(&mut self, record: RecurringRecord) -> Result<()>;
    async fn list_recurrence(&mut self) -> Result<Vec<RecurringRecord>>;
    async fn update_recurrence(&mut self) -> Result<()>;
    async fn list_today(&mut self, include_completed: bool) -> Result<Vec<Record>>;
    async fn list_all(&mut self, include_completed: bool) -> Result<Vec<Record>>;
    async fn events_now(
        &mut self,
        last: chrono::Duration,
        include_completed: bool,
    ) -> Result<Vec<Record>>;
    async fn complete_task(&mut self, primary_key: u64) -> Result<()>;
}

#[async_trait]
pub trait RemoteClient {
    async fn update(&mut self, calendar_id: String, record: Record) -> Result<()>;
    async fn update_recurring(
        &mut self,
        calendar_id: String,
        record: RecurringRecord,
    ) -> Result<()>;
    async fn get(&mut self, calendar_id: String, event_id: String) -> Result<Record>;
    async fn get_recurring(
        &mut self,
        calendar_id: String,
        event_id: String,
    ) -> Result<RecurringRecord>;
    async fn delete(&mut self, calendar_id: String, event_id: String) -> Result<()>;
    async fn delete_recurrence(
        &mut self,
        calendar_id: String,
        event_id: String,
    ) -> Result<Vec<String>>;
    async fn record(&mut self, calendar_id: String, record: Record) -> Result<String>;
    async fn record_recurrence(
        &mut self,
        calendar_id: String,
        record: RecurringRecord,
    ) -> Result<(String, String)>;
    async fn list_recurrence(&mut self, calendar_id: String) -> Result<Vec<RecurringRecord>>;
    async fn update_recurrence(&mut self, calendar_id: String) -> Result<()>;
    async fn list_today(
        &mut self,
        calendar_id: String,
        include_completed: bool,
    ) -> Result<Vec<Record>>;
    async fn list_all(
        &mut self,
        calendar_id: String,
        include_completed: bool,
    ) -> Result<Vec<Record>>;
    async fn events_now(
        &mut self,
        calendar_id: String,
        last: chrono::Duration,
        include_completed: bool,
    ) -> Result<Vec<Record>>;
    async fn complete_task(&mut self, calendar_id: String, primary_key: u64) -> Result<()>;
}
