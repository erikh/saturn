use std::{env::var, path::PathBuf};

pub const CONFIG_FILENAME: &str = ".saturn.conf";
pub const CACHE_FILENAME: &str = ".saturn.cache";
pub const DB_FILENAME: &str = ".saturn.db";

pub fn saturn_config() -> PathBuf {
    dirs::home_dir().unwrap_or("/".into()).join(CONFIG_FILENAME)
}

pub fn saturn_cache() -> PathBuf {
    dirs::home_dir().unwrap_or("/".into()).join(CACHE_FILENAME)
}

pub fn saturn_db() -> PathBuf {
    var("SATURN_DB")
        .unwrap_or(
            dirs::home_dir()
                .unwrap_or("/".into())
                .join(DB_FILENAME)
                .to_str()
                .unwrap()
                .to_string(),
        )
        .into()
}
