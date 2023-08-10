#![allow(dead_code)]
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

    fn delete(&mut self, primary_key: u64) {
        let client = self.client.clone();
        let id = self.lookup(primary_key).expect("Invalid ID");

        tokio::runtime::Handle::current().block_on(async move {
            client
                .expect("Client is not configured properly")
                .delete(id)
                .await
                .unwrap()
        });

        self.remove_by_internal_id(primary_key);
    }

    fn delete_recurrence(&mut self, primary_key: u64) {
        let client = self.client.clone();
        let id = self.lookup(primary_key).expect("Invalid ID");

        tokio::runtime::Handle::current().block_on(async move {
            client
                .expect("Client is not configured properly")
                .delete_recurrence(id)
                .await
                .unwrap()
        });

        self.remove_by_internal_id(primary_key);
    }

    fn record(&mut self, record: Record) -> Result<(), anyhow::Error> {
        let internal = record.internal_key();
        let key = record.primary_key();

        if internal.is_none() {
            return Err(anyhow!("No remote key set"));
        }

        let client = self.client.clone();

        tokio::runtime::Handle::current().block_on(async move {
            client
                .expect("Client is not configured properly")
                .record(record)
                .await
                .unwrap()
        });

        self.add(internal.unwrap(), key);
        Ok(())
    }

    fn record_recurrence(&mut self, record: RecurringRecord) -> Result<(), anyhow::Error> {
        let internal = record.internal_key();
        let recurrence = record.recurrence_key();
        if internal.is_none() {
            return Err(anyhow!("No remote key set"));
        }

        let client = self.client.clone();

        tokio::runtime::Handle::current().block_on(async move {
            client
                .expect("Client is not configured properly")
                .record_recurrence(record)
                .await
                .unwrap()
        });

        self.add_recurring(internal.unwrap(), recurrence);
        Ok(())
    }

    fn list_recurrence(&self) -> Vec<RecurringRecord> {
        Vec::new()
    }

    fn update_recurrence(&mut self) -> Result<(), anyhow::Error> {
        Ok(())
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
