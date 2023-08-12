use crate::{
    db::{RemoteClient, DB},
    record::{Record, RecurringRecord},
};
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct RemoteDB<'a> {
    primary_key: u64,
    recurrence_key: u64,
    id_map: BTreeMap<String, u64>,
    reverse_id_map: BTreeMap<u64, String>,
    recurring_id_map: BTreeMap<String, u64>,
    reverse_recurring_id_map: BTreeMap<u64, String>,
    calendar_id: String,
    #[serde(skip)]
    client: Option<&'a dyn RemoteClient>,
}

impl<'a> RemoteDB<'a> {
    pub fn set_client(&mut self, client: &'a dyn RemoteClient) {
        self.client = Some(client);
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

impl DB for RemoteDB<'_> {
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

    fn delete(&mut self, primary_key: u64) -> Result<(), anyhow::Error> {
        let client = self.client.clone();
        let id = self.lookup(primary_key).expect("Invalid ID");
        let calendar_id = self.calendar_id.clone();

        tokio::runtime::Handle::current().block_on(async move {
            client
                .expect("Client is not configured properly")
                .delete(calendar_id, id)
                .await
        })?;

        self.remove_by_internal_id(primary_key);
        Ok(())
    }

    fn delete_recurrence(&mut self, primary_key: u64) -> Result<(), anyhow::Error> {
        let client = self.client.clone();
        let id = self.lookup(primary_key).expect("Invalid ID");
        let calendar_id = self.calendar_id.clone();

        tokio::runtime::Handle::current().block_on(async move {
            client
                .expect("Client is not configured properly")
                .delete_recurrence(calendar_id, id)
                .await
        })?;

        self.remove_by_internal_id(primary_key);
        Ok(())
    }

    fn record(&mut self, record: Record) -> Result<(), anyhow::Error> {
        let internal = record.internal_key();
        let key = record.primary_key();
        let calendar_id = self.calendar_id.clone();

        if internal.is_none() {
            return Err(anyhow!("No remote key set"));
        }

        let client = self.client.clone();

        tokio::runtime::Handle::current().block_on(async move {
            client
                .expect("Client is not configured properly")
                .record(calendar_id, record)
                .await
        })?;

        self.add(internal.unwrap(), key);
        Ok(())
    }

    fn record_recurrence(&mut self, record: RecurringRecord) -> Result<(), anyhow::Error> {
        let internal = record.internal_key();
        let recurrence = record.recurrence_key();
        let calendar_id = self.calendar_id.clone();

        if internal.is_none() {
            return Err(anyhow!("No remote key set"));
        }

        let client = self.client.clone();

        tokio::runtime::Handle::current().block_on(async move {
            client
                .expect("Client is not configured properly")
                .record_recurrence(calendar_id, record)
                .await
        })?;

        self.add_recurring(internal.unwrap(), recurrence);
        Ok(())
    }

    fn list_recurrence(&self) -> Result<Vec<RecurringRecord>, anyhow::Error> {
        let client = self.client.clone();
        let calendar_id = self.calendar_id.clone();

        tokio::runtime::Handle::current().block_on(async move {
            client
                .expect("Client is not configured properly")
                .list_recurrence(calendar_id)
                .await
        })
    }

    fn update_recurrence(&mut self) -> Result<(), anyhow::Error> {
        let client = self.client.clone();
        let calendar_id = self.calendar_id.clone();

        tokio::runtime::Handle::current().block_on(async move {
            client
                .expect("Client is not configured properly")
                .update_recurrence(calendar_id)
                .await
        })
    }

    fn list_today(&self, include_completed: bool) -> Result<Vec<Record>, anyhow::Error> {
        let client = self.client.clone();
        let calendar_id = self.calendar_id.clone();

        tokio::runtime::Handle::current().block_on(async move {
            client
                .expect("Client is not configured properly")
                .list_today(calendar_id, include_completed)
                .await
        })
    }

    fn list_all(&self, include_completed: bool) -> Result<Vec<Record>, anyhow::Error> {
        let client = self.client.clone();
        let calendar_id = self.calendar_id.clone();

        tokio::runtime::Handle::current().block_on(async move {
            client
                .expect("Client is not configured properly")
                .list_all(calendar_id, include_completed)
                .await
        })
    }

    fn events_now(
        &mut self,
        last: chrono::Duration,
        include_completed: bool,
    ) -> Result<Vec<Record>, anyhow::Error> {
        let client = self.client.clone();
        let calendar_id = self.calendar_id.clone();

        tokio::runtime::Handle::current().block_on(async move {
            client
                .expect("Client is not configured properly")
                .events_now(calendar_id, last, include_completed)
                .await
        })
    }

    fn complete_task(&mut self, primary_key: u64) -> Result<(), anyhow::Error> {
        let client = self.client.clone();
        let calendar_id = self.calendar_id.clone();

        tokio::runtime::Handle::current().block_on(async move {
            client
                .expect("Client is not configured properly")
                .complete_task(calendar_id, primary_key)
                .await
        })
    }
}
