use crate::{
    config::{Config, DBType},
    record::{Record, RecordType},
};
use anyhow::anyhow;
use chrono::Duration;
use fancy_duration::FancyDuration;
use gcal::{oauth_listener, oauth_user_url, ClientParameters, State};
use tokio::sync::Mutex;

pub fn sort_records(a: &Record, b: &Record) -> std::cmp::Ordering {
    let cmp = a.date().cmp(&b.date());
    if cmp == std::cmp::Ordering::Equal {
        match a.record_type() {
            RecordType::At => {
                if let Some(a_at) = a.at() {
                    if let Some(b_at) = b.at() {
                        a_at.cmp(&b_at)
                    } else if let Some(b_schedule) = b.scheduled() {
                        a_at.cmp(&b_schedule.0)
                    } else {
                        std::cmp::Ordering::Equal
                    }
                } else {
                    std::cmp::Ordering::Equal
                }
            }
            RecordType::AllDay => {
                if b.record_type() == RecordType::AllDay {
                    a.primary_key().cmp(&b.primary_key())
                } else {
                    std::cmp::Ordering::Less
                }
            }
            RecordType::Schedule => {
                if let Some(a_schedule) = a.scheduled() {
                    if let Some(b_schedule) = b.scheduled() {
                        a_schedule.0.cmp(&b_schedule.0)
                    } else if let Some(b_at) = b.at() {
                        a_schedule.0.cmp(&b_at)
                    } else {
                        std::cmp::Ordering::Equal
                    }
                } else {
                    std::cmp::Ordering::Equal
                }
            }
        }
    } else {
        cmp
    }
}

pub fn get_config() -> Result<Config, anyhow::Error> {
    Config::load(None)
}

pub fn set_db_type(db_type: String) -> Result<(), anyhow::Error> {
    let mut config = get_config()?;
    let typ = match db_type.as_str() {
        "google" => DBType::Google,
        "unixfile" => DBType::UnixFile,
        _ => {
            return Err(anyhow!(
                "Invalid db type: valid types are `google` and `unixfile`"
            ))
        }
    };

    config.set_db_type(typ);
    config.save(None)?;

    Ok(())
}

pub async fn get_access_token() -> Result<(), anyhow::Error> {
    let mut config = get_config()?;

    if !config.has_client() {
        return Err(anyhow!(
            "You need to configure a client first; see `saturn config set-client`"
        ));
    }

    let mut params = ClientParameters {
        client_id: config.client_id().unwrap(),
        client_secret: config.client_secret().unwrap(),
        ..Default::default()
    };

    let state = State::new(Mutex::new(params.clone()));
    let host = oauth_listener(state.clone()).await?;
    params.redirect_url = Some(format!("http://{}", host));

    let url = oauth_user_url(params.clone());
    println!("Click on this and login: {}", url);

    loop {
        let lock = state.lock().await;
        if lock.access_key.is_some() {
            config.set_access_token(lock.access_key.clone());
            config.set_access_token_expires_at(lock.expires_at);
            config.set_refresh_token(lock.refresh_token.clone());
            config.set_refresh_token_expires_at(lock.refresh_token_expires_at);
            config.set_redirect_url(params.redirect_url.clone());
            config.save(None)?;
            println!("Captured. Thanks!");
            return Ok(());
        }

        tokio::time::sleep(std::time::Duration::new(1, 0)).await;
    }
}

pub fn set_client_info(client_id: String, client_secret: String) -> Result<(), anyhow::Error> {
    let mut config = get_config()?;
    config.set_client_info(client_id, client_secret);
    config.save(None)
}

pub fn set_sync_window(duration: FancyDuration<Duration>) -> Result<(), anyhow::Error> {
    let mut config = get_config()?;
    config.set_sync_duration(Some(duration));
    config.save(None)
}
