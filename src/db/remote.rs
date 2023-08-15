use crate::{
    db::{unixfile::UnixFileLoader, RemoteClient, DB},
    filenames::saturn_db,
    record::{Record, RecurringRecord},
};
use anyhow::anyhow;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct RemoteDB<T: RemoteClient + Send + Sync + Default> {
    primary_key: u64,
    recurrence_key: u64,
    id_map: BTreeMap<String, u64>,
    reverse_id_map: BTreeMap<u64, String>,
    recurring_id_map: BTreeMap<String, u64>,
    reverse_recurring_id_map: BTreeMap<u64, String>,
    calendar_id: String,
    #[serde(skip)]
    client: Option<T>,
}

impl<T: RemoteClient + Send + Sync + Default> RemoteDB<T> {
    pub fn new(calendar_id: String, client: T) -> Self {
        // assuming this call convention is honored, client will always be "some" when actually
        // used, and will only be empty when deserialized.
        Self {
            client: Some(client),
            primary_key: 0,
            recurrence_key: 0,
            id_map: BTreeMap::default(),
            reverse_id_map: BTreeMap::default(),
            recurring_id_map: BTreeMap::default(),
            reverse_recurring_id_map: BTreeMap::default(),
            calendar_id,
        }
    }

    pub fn add_internal(&mut self, primary_key: u64, remote_key: String) {
        self.id_map.insert(remote_key.clone(), primary_key);
        self.reverse_id_map.insert(primary_key, remote_key);
    }

    pub fn add(&mut self, primary_key: String, remote_key: u64) {
        self.reverse_id_map.insert(remote_key, primary_key.clone());
        self.id_map.insert(primary_key, remote_key);
    }

    pub fn add_recurring_internal(&mut self, primary_key: u64, remote_key: String) {
        self.recurring_id_map
            .insert(remote_key.clone(), primary_key);
        self.reverse_recurring_id_map
            .insert(primary_key, remote_key);
    }

    pub fn add_recurring(&mut self, primary_key: String, remote_key: u64) {
        self.reverse_recurring_id_map
            .insert(remote_key, primary_key.clone());
        self.recurring_id_map.insert(primary_key, remote_key);
    }

    pub fn lookup_internal(&self, id: String) -> Option<u64> {
        self.id_map.get(&id).cloned()
    }

    pub fn lookup(&self, id: u64) -> Option<String> {
        self.reverse_id_map.get(&id).cloned()
    }

    pub fn recurring_lookup_internal(&self, id: String) -> Option<u64> {
        self.recurring_id_map.get(&id).cloned()
    }

    pub fn recurring_lookup(&self, id: u64) -> Option<String> {
        self.reverse_recurring_id_map.get(&id).cloned()
    }

    pub fn remove_by_internal_id(&mut self, id: u64) {
        self.reverse_id_map
            .remove(&id)
            .map(|o| self.id_map.remove(&o));
    }

    pub fn remove_by_public_id(&mut self, id: String) {
        self.id_map
            .remove(&id)
            .map(|o| self.reverse_id_map.remove(&o));
    }

    pub fn remove_recurring_by_internal_id(&mut self, id: u64) {
        self.reverse_recurring_id_map
            .remove(&id)
            .map(|o| self.recurring_id_map.remove(&o));
    }

    pub fn remove_recurring_by_public_id(&mut self, id: String) {
        self.recurring_id_map
            .remove(&id)
            .map(|o| self.reverse_recurring_id_map.remove(&o));
    }
}

impl<T: RemoteClient + Send + Sync + Clone + Default> RemoteDB<T> {
    fn record_internal(
        &mut self,
        internal_key: String,
        pk: Option<u64>,
        record: &mut Record,
    ) -> Result<(), anyhow::Error> {
        if let Some(pk) = pk {
            record.set_primary_key(pk);
        } else {
            let key = self.primary_key();
            record.set_primary_key(key);
            self.set_primary_key(key + 1);
        }
        self.add_internal(record.primary_key(), internal_key);
        Ok(())
    }

    fn record_internal_recurrence(
        &mut self,
        internal_recurrence_key: String,
        recurrence_key: u64,
    ) -> Result<(), anyhow::Error> {
        self.add_recurring_internal(recurrence_key, internal_recurrence_key);
        Ok(())
    }

    async fn record_updates(
        &mut self,
        mut records: Vec<Record>,
    ) -> Result<Vec<Record>, anyhow::Error> {
        for record in &mut records {
            if let Some(internal_recurrence_key) = record.internal_recurrence_key() {
                let key = self.recurrence_key();
                self.record_internal_recurrence(internal_recurrence_key, key)?;
                self.set_recurrence_key(key + 1);
            }

            if let Some(internal_key) = record.internal_key() {
                let pk = self.lookup_internal(internal_key.clone());
                self.record_internal(internal_key, pk, record)?;
            } else {
                self.record(record.clone()).await?;
                self.record_internal(record.internal_key().unwrap(), None, record)?;
            }
        }

        Ok(records)
    }

    async fn record_recurring_updates(
        &mut self,
        records: Vec<RecurringRecord>,
    ) -> Result<Vec<RecurringRecord>, anyhow::Error> {
        for record in &records {
            self.record_recurrence(record.clone()).await?;
        }
        Ok(records)
    }
}
#[async_trait]
impl<T: RemoteClient + Send + Sync + Clone + Default> DB for RemoteDB<T> {
    async fn load(&mut self) -> Result<(), anyhow::Error> {
        let db: Self = UnixFileLoader::new(&saturn_db()).load().await;
        self.primary_key = db.primary_key;
        self.recurrence_key = db.recurrence_key;
        self.id_map = db.id_map;
        self.reverse_id_map = db.reverse_id_map;
        self.recurring_id_map = db.recurring_id_map;
        self.update_recurrence().await
    }

    async fn dump(&self) -> Result<(), anyhow::Error> {
        UnixFileLoader::new(&saturn_db()).dump(self.clone()).await
    }

    fn primary_key(&self) -> u64 {
        self.primary_key
    }

    fn set_primary_key(&mut self, primary_key: u64) {
        self.primary_key = primary_key;
    }

    fn recurrence_key(&self) -> u64 {
        self.recurrence_key
    }

    fn set_recurrence_key(&mut self, primary_key: u64) {
        self.recurrence_key = primary_key;
    }

    async fn delete(&mut self, primary_key: u64) -> Result<(), anyhow::Error> {
        let id = self.lookup(primary_key).expect("Invalid ID");
        let calendar_id = self.calendar_id.clone();

        self.client.clone().unwrap().delete(calendar_id, id).await?;

        self.remove_by_internal_id(primary_key);
        Ok(())
    }

    async fn delete_recurrence(&mut self, primary_key: u64) -> Result<(), anyhow::Error> {
        let id = self.lookup(primary_key).expect("Invalid ID");
        let calendar_id = self.calendar_id.clone();

        self.client
            .clone()
            .unwrap()
            .delete_recurrence(calendar_id, id)
            .await?;

        self.remove_by_internal_id(primary_key);
        Ok(())
    }

    async fn record(&mut self, record: Record) -> Result<(), anyhow::Error> {
        if !self.reverse_id_map.contains_key(&record.primary_key()) {
            self.insert_record(record).await
        } else {
            Ok(())
        }
    }

    async fn record_recurrence(&mut self, record: RecurringRecord) -> Result<(), anyhow::Error> {
        if !self
            .reverse_recurring_id_map
            .contains_key(&record.recurrence_key())
        {
            self.insert_recurrence(record).await
        } else {
            Ok(())
        }
    }

    async fn insert_record(&mut self, record: Record) -> Result<(), anyhow::Error> {
        let key = record.primary_key();
        let calendar_id = self.calendar_id.clone();

        let internal_key = self
            .client
            .clone()
            .unwrap()
            .record(calendar_id, record)
            .await?;

        self.add(internal_key, key);
        Ok(())
    }

    async fn insert_recurrence(&mut self, record: RecurringRecord) -> Result<(), anyhow::Error> {
        let internal = record.internal_key();
        let recurrence = record.recurrence_key();
        let calendar_id = self.calendar_id.clone();

        if internal.is_none() {
            return Err(anyhow!("No remote key set"));
        }

        self.client
            .clone()
            .unwrap()
            .record_recurrence(calendar_id, record)
            .await?;

        self.add_recurring(internal.unwrap(), recurrence);
        Ok(())
    }

    async fn list_recurrence(&mut self) -> Result<Vec<RecurringRecord>, anyhow::Error> {
        let calendar_id = self.calendar_id.clone();

        self.record_recurring_updates(
            self.client
                .clone()
                .unwrap()
                .list_recurrence(calendar_id)
                .await?,
        )
        .await
    }

    async fn update_recurrence(&mut self) -> Result<(), anyhow::Error> {
        let calendar_id = self.calendar_id.clone();

        self.client
            .clone()
            .unwrap()
            .update_recurrence(calendar_id)
            .await
    }

    async fn list_today(&mut self, include_completed: bool) -> Result<Vec<Record>, anyhow::Error> {
        let calendar_id = self.calendar_id.clone();

        self.record_updates(
            self.client
                .clone()
                .unwrap()
                .list_today(calendar_id, include_completed)
                .await?,
        )
        .await
    }

    async fn list_all(&mut self, include_completed: bool) -> Result<Vec<Record>, anyhow::Error> {
        let calendar_id = self.calendar_id.clone();

        self.record_updates(
            self.client
                .clone()
                .unwrap()
                .list_all(calendar_id, include_completed)
                .await?,
        )
        .await
    }

    async fn events_now(
        &mut self,
        last: chrono::Duration,
        include_completed: bool,
    ) -> Result<Vec<Record>, anyhow::Error> {
        let calendar_id = self.calendar_id.clone();

        self.record_updates(
            self.client
                .clone()
                .unwrap()
                .events_now(calendar_id, last, include_completed)
                .await?,
        )
        .await
    }

    async fn complete_task(&mut self, primary_key: u64) -> Result<(), anyhow::Error> {
        let calendar_id = self.calendar_id.clone();

        self.client
            .clone()
            .unwrap()
            .complete_task(calendar_id, primary_key)
            .await
    }
}
