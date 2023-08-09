use chrono::Duration;
use fancy_duration::FancyDuration;
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
pub struct Config {
    db_type: DBType,
    access_token: Option<String>,
    client_info: Option<(String, String)>,
    sync_duration: Option<FancyDuration<Duration>>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            db_type: DBType::UnixFile,
            access_token: None,
            client_info: None,
            sync_duration: None,
        }
    }
}

impl Config {
    pub fn load(filename: std::path::PathBuf) -> Result<Self, anyhow::Error> {
        let mut io = std::fs::OpenOptions::new();
        io.read(true);

        match io.open(filename) {
            Ok(io) => Ok(serde_yaml::from_reader(io)?),
            Err(_) => Ok(Self::default()),
        }
    }

    pub fn save(&self, filename: std::path::PathBuf) -> Result<(), anyhow::Error> {
        let mut io = std::fs::OpenOptions::new();
        io.write(true);
        io.truncate(true);
        io.create(true);
        let io = io.open(filename)?;

        Ok(serde_yaml::to_writer(io, self)?)
    }

    pub fn set_access_token(&mut self, access_token: Option<String>) {
        self.access_token = access_token;
    }

    pub fn access_token(&self) -> Option<String> {
        self.access_token.clone()
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
