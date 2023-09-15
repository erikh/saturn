use crate::config::{CONFIG_FILENAME, DB_FILENAME};
use std::{env::var, path::PathBuf};

pub fn saturn_config() -> PathBuf {
    dirs::home_dir().unwrap_or("/".into()).join(CONFIG_FILENAME)
}

pub fn saturn_db() -> PathBuf {
    PathBuf::from(
        var("SATURN_DB").unwrap_or(
            dirs::home_dir()
                .unwrap_or("/".into())
                .join(DB_FILENAME)
                .to_str()
                .unwrap()
                .to_string(),
        ),
    )
}
