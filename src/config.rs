use crate::filenames::saturn_config;
use chrono::Duration;
use fancy_duration::FancyDuration;
use gcal::ClientParameters;
use serde::{Deserialize, Serialize};

pub const CONFIG_FILENAME: &str = ".saturn.conf";
pub const DB_FILENAME: &str = ".saturn.db";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum DBType {
    #[default]
    UnixFile,
    Google,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
// FIXME add default duration for calendar items
pub struct Config {
    db_type: DBType,
    access_token: Option<String>,
    access_token_expires_at: Option<chrono::NaiveDateTime>,
    refresh_token: Option<String>,
    refresh_token_expires_at: Option<chrono::NaiveDateTime>,
    client_info: Option<(String, String)>,
    redirect_url: Option<String>,
    sync_duration: Option<FancyDuration<Duration>>,
    calendar_id: String,
}

impl From<Config> for ClientParameters {
    fn from(value: Config) -> Self {
        Self {
            client_id: value.client_id().unwrap_or_default(),
            client_secret: value.client_secret().unwrap_or_default(),
            redirect_url: value.redirect_url(),
            access_key: value.access_token(),
            expires_at: value.access_token_expires_at(),
            refresh_token: value.refresh_token(),
            refresh_token_expires_at: value.refresh_token_expires_at(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            db_type: DBType::UnixFile,
            access_token: None,
            access_token_expires_at: None,
            refresh_token: None,
            refresh_token_expires_at: None,
            redirect_url: None,
            client_info: None,
            sync_duration: None,
            calendar_id: "primary".to_string(),
        }
    }
}

impl Config {
    pub fn load(filename: Option<std::path::PathBuf>) -> Result<Self, anyhow::Error> {
        let path = filename.unwrap_or(saturn_config());
        let mut io = std::fs::OpenOptions::new();
        io.read(true);

        match io.open(path) {
            Ok(io) => Ok(serde_yaml::from_reader(io)?),
            Err(_) => Ok(Self::default()),
        }
    }

    pub fn save(&self, filename: Option<std::path::PathBuf>) -> Result<(), anyhow::Error> {
        let path = filename.unwrap_or(saturn_config());
        let mut io = std::fs::OpenOptions::new();
        io.write(true);
        io.truncate(true);
        io.create(true);
        let io = io.open(path)?;

        Ok(serde_yaml::to_writer(io, self)?)
    }

    pub fn set_calendar_id(&mut self, calendar_id: String) {
        self.calendar_id = calendar_id;
    }

    pub fn set_access_token(&mut self, access_token: Option<String>) {
        self.access_token = access_token;
    }

    pub fn set_access_token_expires_at(&mut self, expires_at: Option<chrono::NaiveDateTime>) {
        self.access_token_expires_at = expires_at;
    }

    pub fn set_refresh_token(&mut self, refresh_token: Option<String>) {
        self.refresh_token = refresh_token;
    }

    pub fn set_refresh_token_expires_at(&mut self, expires_at: Option<chrono::NaiveDateTime>) {
        self.refresh_token_expires_at = expires_at;
    }

    pub fn access_token(&self) -> Option<String> {
        self.access_token.clone()
    }

    pub fn access_token_expires_at(&self) -> Option<chrono::NaiveDateTime> {
        self.access_token_expires_at
    }

    pub fn refresh_token(&self) -> Option<String> {
        self.refresh_token.clone()
    }

    pub fn refresh_token_expires_at(&self) -> Option<chrono::NaiveDateTime> {
        self.refresh_token_expires_at
    }

    pub fn set_redirect_url(&mut self, redirect_url: Option<String>) {
        self.redirect_url = redirect_url;
    }

    pub fn calendar_id(&self) -> String {
        self.calendar_id.clone()
    }

    pub fn redirect_url(&self) -> Option<String> {
        self.redirect_url.clone()
    }

    pub fn set_db_type(&mut self, typ: DBType) {
        self.db_type = typ;
    }

    pub fn db_type(&self) -> DBType {
        self.db_type.clone()
    }

    pub fn set_sync_duration(&mut self, sync_duration: Option<FancyDuration<Duration>>) {
        self.sync_duration = sync_duration;
    }

    pub fn sync_duration(&self) -> Option<FancyDuration<Duration>> {
        self.sync_duration.clone()
    }

    pub fn set_client_info(&mut self, client_id: String, client_secret: String) {
        self.client_info = Some((client_id, client_secret))
    }

    pub fn has_client(&self) -> bool {
        self.client_info.is_some()
    }

    pub fn client_id(&self) -> Option<String> {
        self.client_info.clone().map(|s| s.0)
    }

    pub fn client_secret(&self) -> Option<String> {
        self.client_info.clone().map(|s| s.1)
    }
}
