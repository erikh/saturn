use crate::config::{CONFIG_FILENAME, DB_FILENAME};
use std::{env::var, path::PathBuf};

pub fn saturn_config() -> PathBuf {
    PathBuf::from(var("HOME").unwrap_or("/".to_string())).join(CONFIG_FILENAME)
}

pub fn saturn_db<'a>() -> PathBuf {
    PathBuf::from(
        var("SATURN_DB").unwrap_or(
            PathBuf::from(var("HOME").unwrap_or("/".to_string()))
                .join(DB_FILENAME)
                .to_str()
                .unwrap()
                .to_string(),
        ),
    )
}
