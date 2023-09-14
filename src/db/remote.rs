use crate::{
    db::{unixfile::UnixFileLoader, RemoteClient, DB},
    filenames::saturn_db,
    record::{Record, RecurringRecord},
};
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct RemoteDBClient<T: RemoteClient + Send + Sync + Default + std::fmt::Debug> {
    client: T,
    db: RemoteDB,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RemoteDB {
    primary_key: u64,
    recurrence_key: u64,
    id_map: BTreeMap<String, u64>,
    reverse_id_map: BTreeMap<u64, String>,
    recurring_id_map: BTreeMap<String, u64>,
    reverse_recurring_id_map: BTreeMap<u64, String>,
    calendar_id: String,
}

impl<T: RemoteClient + Send + Sync + Default + std::fmt::Debug> RemoteDBClient<T> {
    pub fn new(calendar_id: String, client: T) -> Self {
        let db = RemoteDB::new(calendar_id);

        // assuming this call convention is honored, client will always be "some" when actually
        // used, and will only be empty when deserialized.
        Self { client, db }
    }
}

impl RemoteDB {
    pub fn new(calendar_id: String) -> Self {
        Self {
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

impl RemoteDB {
    fn record_internal(&mut self, internal_key: String, pk: Option<u64>) -> Result<u64> {
        let pk = if let Some(pk) = pk {
            pk
        } else {
            self.next_key()
        };

        self.add_internal(pk, internal_key);
        Ok(pk)
    }

    async fn record_updates(&mut self, mut records: Vec<Record>) -> Result<Vec<Record>> {
        for record in &mut records {
            if let Some(internal_recurrence_key) = record.internal_recurrence_key() {
                if record.recurrence_key().is_none() {
                    let key = self.next_recurrence_key();
                    record.set_recurrence_key(Some(key));
                    self.add_recurring(internal_recurrence_key, key);
                }
            }

            if let Some(internal_key) = record.internal_key() {
                if record.primary_key() == 0 {
                    record.set_primary_key(self.record_internal(
                        internal_key.clone(),
                        self.lookup_internal(internal_key),
                    )?);
                }
            }
        }

        Ok(records)
    }

    async fn record_recurring_updates(
        &mut self,
        mut records: Vec<RecurringRecord>,
    ) -> Result<Vec<RecurringRecord>> {
        let mut v = Vec::new();
        for record in &mut records {
            if let Some(internal_recurrence_key) = record.internal_key() {
                if let Some(internal) =
                    self.recurring_lookup_internal(internal_recurrence_key.clone())
                {
                    record.set_recurrence_key(internal);
                    record.record().set_recurrence_key(Some(internal));
                    self.add_recurring(internal_recurrence_key.clone(), internal);
                } else if record.recurrence_key() == 0 {
                    let key = self.next_recurrence_key();
                    record.set_recurrence_key(key);
                    record.record().set_recurrence_key(Some(key));
                    self.add_recurring(internal_recurrence_key.clone(), key);
                } else {
                    record.recurrence_key();
                }
            }

            if let Some(internal_key) = record.record().internal_key() {
                if record.record().primary_key() == 0 {
                    record.record().set_primary_key(self.record_internal(
                        internal_key.clone(),
                        self.lookup_internal(internal_key),
                    )?);
                }
            }

            v.push(record.clone());
        }

        Ok(v)
    }
}

#[async_trait]
impl DB for RemoteDB {
    async fn load(&mut self) -> Result<()> {
        let db: Self = UnixFileLoader::new(&saturn_db()).load().await;
        self.primary_key = db.primary_key;
        self.recurrence_key = db.recurrence_key;
        self.id_map = db.id_map;
        self.reverse_id_map = db.reverse_id_map;
        self.recurring_id_map = db.recurring_id_map;
        self.reverse_recurring_id_map = db.reverse_recurring_id_map;
        self.update_recurrence().await
    }

    async fn dump(&self) -> Result<()> {
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

    fn set_recurrence_key(&mut self, recurrence_key: u64) {
        self.recurrence_key = recurrence_key;
    }

    async fn delete(&mut self, primary_key: u64) -> Result<()> {
        self.remove_by_internal_id(primary_key);
        Ok(())
    }

    async fn delete_recurrence(&mut self, recurrence_key: u64) -> Result<Vec<String>> {
        self.remove_by_internal_id(recurrence_key);
        self.remove_recurring_by_internal_id(recurrence_key);
        // FIXME leaves a garbage record in the PK table
        Ok(Vec::new())
    }

    async fn record(&mut self, _record: Record) -> Result<()> {
        Ok(())
    }

    async fn record_recurrence(&mut self, _record: RecurringRecord) -> Result<()> {
        Ok(())
    }

    async fn insert_record(&mut self, _record: Record) -> Result<()> {
        Ok(())
    }

    async fn insert_recurrence(&mut self, _record: RecurringRecord) -> Result<()> {
        Ok(())
    }

    async fn list_recurrence(&mut self) -> Result<Vec<RecurringRecord>> {
        Ok(Default::default())
    }

    async fn update_recurrence(&mut self) -> Result<()> {
        Ok(())
    }

    async fn list_today(&mut self, _include_completed: bool) -> Result<Vec<Record>> {
        Ok(Default::default())
    }

    async fn list_all(&mut self, _include_completed: bool) -> Result<Vec<Record>> {
        Ok(Default::default())
    }

    async fn events_now(
        &mut self,
        _last: chrono::Duration,
        _include_completed: bool,
    ) -> Result<Vec<Record>> {
        Ok(Default::default())
    }

    async fn complete_task(&mut self, _primary_key: u64) -> Result<()> {
        Ok(())
    }
}

#[async_trait]
impl<T: RemoteClient + Send + Sync + Default + std::fmt::Debug> DB for RemoteDBClient<T> {
    async fn load(&mut self) -> Result<()> {
        self.db.load().await
    }

    async fn dump(&self) -> Result<()> {
        self.db.dump().await
    }

    fn primary_key(&self) -> u64 {
        self.db.primary_key()
    }

    fn set_primary_key(&mut self, primary_key: u64) {
        self.db.set_primary_key(primary_key)
    }

    fn recurrence_key(&self) -> u64 {
        self.db.recurrence_key()
    }

    fn set_recurrence_key(&mut self, recurrence_key: u64) {
        self.db.set_recurrence_key(recurrence_key);
    }

    async fn delete(&mut self, primary_key: u64) -> Result<()> {
        let id = self.db.lookup(primary_key).expect("Invalid ID");
        let calendar_id = self.db.calendar_id.clone();

        self.client.delete(calendar_id, id).await?;
        self.db.delete(primary_key).await?;
        Ok(())
    }

    async fn delete_recurrence(&mut self, recurrence_key: u64) -> Result<Vec<String>> {
        let id = self
            .db
            .recurring_lookup(recurrence_key)
            .expect("Invalid ID");
        let calendar_id = self.db.calendar_id.clone();

        let list = self
            .client
            .delete_recurrence(calendar_id.clone(), id.clone())
            .await?;
        for item in list.iter() {
            if let Some(id) = self.db.lookup_internal(item.clone()) {
                let res = self.delete(id).await;
                if matches!(res, Result::Err(_)) {
                    break;
                }
            }
        }

        self.db.delete_recurrence(recurrence_key).await?;
        if let Some(id) = self.db.lookup_internal(id) {
            self.db.delete(id).await?;
        }
        // FIXME leaves a garbage record in the PK table
        Ok(list)
    }

    async fn record(&mut self, record: Record) -> Result<()> {
        if let None = self.db.lookup(record.primary_key()) {
            self.insert_record(record).await
        } else {
            Ok(())
        }
    }

    async fn record_recurrence(&mut self, record: RecurringRecord) -> Result<()> {
        if let None = self.db.recurring_lookup(record.recurrence_key()) {
            self.insert_recurrence(record).await
        } else {
            Ok(())
        }
    }

    async fn insert_record(&mut self, record: Record) -> Result<()> {
        let key = record.primary_key();
        let calendar_id = self.db.calendar_id.clone();

        let internal_key = self.client.record(calendar_id, record).await?;

        self.db.add(internal_key, key);
        Ok(())
    }

    async fn insert_recurrence(&mut self, mut record: RecurringRecord) -> Result<()> {
        let calendar_id = self.db.calendar_id.clone();

        let (key, recurrence_key) = self
            .client
            .record_recurrence(calendar_id, record.clone())
            .await?;

        record.set_internal_key(Some(key.clone()));
        record
            .record()
            .set_internal_recurrence_key(Some(key.clone()));
        record.record().set_internal_key(Some(key.clone()));

        if record.recurrence_key() == 0 {
            record.set_recurrence_key(self.next_recurrence_key());
            record
                .record()
                .set_recurrence_key(Some(self.recurrence_key()));
        }

        record.record().set_primary_key(self.next_key());

        self.insert_record(record.record().clone()).await?;
        self.db
            .add_recurring(recurrence_key, record.recurrence_key());
        Ok(())
    }

    async fn list_recurrence(&mut self) -> Result<Vec<RecurringRecord>> {
        let calendar_id = self.db.calendar_id.clone();

        self.db
            .record_recurring_updates(self.client.list_recurrence(calendar_id).await?)
            .await
    }

    async fn update_recurrence(&mut self) -> Result<()> {
        let calendar_id = self.db.calendar_id.clone();

        self.client.update_recurrence(calendar_id).await
    }

    async fn list_today(&mut self, include_completed: bool) -> Result<Vec<Record>> {
        let calendar_id = self.db.calendar_id.clone();

        self.db
            .record_updates(
                self.client
                    .list_today(calendar_id, include_completed)
                    .await?,
            )
            .await
    }

    async fn list_all(&mut self, include_completed: bool) -> Result<Vec<Record>> {
        let calendar_id = self.db.calendar_id.clone();

        self.db
            .record_updates(self.client.list_all(calendar_id, include_completed).await?)
            .await
    }

    async fn events_now(
        &mut self,
        last: chrono::Duration,
        include_completed: bool,
    ) -> Result<Vec<Record>> {
        let calendar_id = self.db.calendar_id.clone();

        self.db
            .record_updates(
                self.client
                    .events_now(calendar_id, last, include_completed)
                    .await?,
            )
            .await
    }

    async fn complete_task(&mut self, primary_key: u64) -> Result<()> {
        let calendar_id = self.db.calendar_id.clone();

        self.client.complete_task(calendar_id, primary_key).await
    }
}
