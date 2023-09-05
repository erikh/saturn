use crate::{
    cli::get_config,
    config::{Config, DBType},
    db::{google::GoogleClient, memory::MemoryDB, remote::RemoteDB, DB},
    list_ui, process_ui_command,
    record::Record,
};
use ratatui::widgets::*;
use std::{sync::Arc, time::Duration};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Default)]
pub struct State<'a> {
    pub records: Vec<Record>,
    pub list_type: super::types::ListType,
    pub notification: Option<(String, chrono::NaiveDateTime)>,
    pub line_buf: String,
    pub command: Option<super::types::CommandType>,
    pub calendar: Option<(Arc<Table<'a>>, chrono::NaiveDateTime)>,
    pub events: Option<(Arc<Table<'a>>, chrono::NaiveDateTime)>,
    pub redraw: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ProtectedState<'a>(Arc<Mutex<State<'a>>>);

impl<'a> std::ops::Deref for ProtectedState<'a> {
    type Target = Arc<Mutex<State<'a>>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a> ProtectedState<'a> {
    pub async fn list_google(&self, config: Config) -> Result<Vec<Record>, anyhow::Error> {
        let client = GoogleClient::new(config.clone())?;

        let mut db = RemoteDB::new(config.calendar_id(), client.clone());
        let list_type = self.lock().await.list_type.clone();
        list_ui!(db, list_type)
    }

    pub async fn list_file(&self) -> Result<Vec<Record>, anyhow::Error> {
        let mut db = MemoryDB::new();
        let list_type = self.lock().await.list_type.clone();
        list_ui!(db, list_type)
    }

    pub async fn command_google(&self, config: Config) -> Result<(), anyhow::Error> {
        let client = GoogleClient::new(config.clone())?;

        let mut db = RemoteDB::new(config.calendar_id(), client.clone());
        let command = self.lock().await.command.clone();
        process_ui_command!(db, command);
        self.lock().await.command = None;
        Ok(())
    }

    pub async fn command_file(&self) -> Result<(), anyhow::Error> {
        let mut db = MemoryDB::new();
        let command = self.lock().await.command.clone();
        process_ui_command!(db, command);
        self.lock().await.command = None;
        Ok(())
    }

    pub async fn update_state(&self) -> Result<(), anyhow::Error> {
        let config = get_config().unwrap_or_default();

        let typ = config.db_type();

        match typ {
            DBType::UnixFile => self.command_file().await,
            DBType::Google => self.command_google(config.clone()).await,
        }
        .expect("Could not execute command");

        let mut list = match typ {
            DBType::UnixFile => self.list_file().await,
            DBType::Google => self.list_google(config).await,
        }
        .expect("Could not read DB");

        let mut inner = self.lock().await;
        inner.records.clear();
        inner.records.append(&mut list);
        inner.redraw = true;
        Ok(())
    }

    pub async fn refresh(&self) -> Result<(), anyhow::Error> {
        loop {
            self.update_state().await?;
            tokio::time::sleep(Duration::new(60, 0)).await;
        }
    }

    pub async fn add_notification(&self, notification: &str) {
        self.lock().await.notification =
            Some((notification.to_string(), chrono::Local::now().naive_local()))
    }
}
