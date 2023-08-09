use serde::{Deserialize, Serialize};

pub const CONFIG_FILENAME: &str = ".saturn.conf";
pub const DB_FILENAME: &str = ".saturn.db";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum DBType {
    #[default]
    UnixFile,
    Google,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    db_type: DBType,
    access_token: Option<String>,
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
}
